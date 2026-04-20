# fourfour Benchmark — Implementation Plan

> **Status: Historical reference.** Benchmarking was completed externally in the samplebase project. The results, library recommendations, and accuracy data are captured in [`analysis-pipeline-handoff.md`](./analysis-pipeline-handoff.md). This document is retained as a design reference for any future benchmark work inside fourfour.

---

## What Happened

Benchmarking of audio analysis libraries (BPM, key, energy, waveform, embeddings) was done in the **samplebase** project (`~/dev/projects/samplebase`), not in this repo. The research produced:

- Accuracy benchmarks for BPM detection (DeepRhythm ~97% Acc2), key detection (librosa + KS ~70%), and energy scoring (validated across 19 genres)
- Embedding model comparisons (MS CLAP, LAION-CLAP, TTMR++, CLaMP 3)
- A full Python analysis pipeline design with concrete code for every component

**The output is in [`analysis-pipeline-handoff.md`](./analysis-pipeline-handoff.md)** — that's the authoritative doc for what to build.

### What was NOT built in this project

- No Python benchmark harness exists in `analysis/`
- No ground truth extraction from Rekordbox master.db / ANLZ
- No `fourfour-benchmark` CLI
- No `StratumDspBackend` adapter or `AnalysisBackend` ABC

These may be built later if needed, but the immediate path is to implement the Python analysis CLI described in the handoff doc.

---

## Original Plan (Retained for Reference)

## Current Repo Architecture (as of v0.9.0)

```
fourfour/
├── Cargo.toml                    # workspace: 3 crates
├── pioneer-usb-writer/           # Pure format library (no CLI, no analyzer)
│   └── src/
│       ├── lib.rs                # Re-exports: models, reader, scanner, writer
│       ├── models.rs             # Track, AnalysisResult, BeatGrid, Beat, CuePoint,
│       │                         # ExistingTrack, ExistingUsbState, SyncPlan, etc.
│       ├── scanner.rs            # lofty-based tag reader
│       ├── reader/
│       │   ├── mod.rs
│       │   ├── usb.rs            # Reads OneLibrary exportLibrary.db from USB
│       │   └── masterdb.rs       # Reads Rekordbox ~/Library/Pioneer/rekordbox/master.db
│       └── writer/
│           ├── anlz.rs           # ANLZ0000.DAT/EXT writer (beat grids, waveforms, cues)
│           ├── pdb.rs            # export.pdb writer (legacy DeviceSQL)
│           ├── onelibrary.rs     # exportLibrary.db writer (SQLCipher)
│           ├── filesystem.rs     # Orchestrator: copy audio + write all format files
│           ├── sync.rs           # Incremental sync: diff tracks vs existing USB state
│           └── mod.rs
├── pioneer-test-ui/              # Tauri v2 test harness
│   └── src/
│       ├── main.rs               # Tauri app + all commands
│       ├── analyzer/
│       │   ├── mod.rs            # stratum-dsp + symphonia: BPM, key, beats, waveform
│       │   └── waveform.rs       # 400-byte monochrome waveform preview
│       └── dto.rs                # Frontend DTOs
└── pioneer-library/              # Persistent local library (SQLite CRUD + export/sync)
    └── src/
        ├── lib.rs                # LocalLibrary: add_track, get_analysis, export_to_usb, etc.
        ├── queries.rs            # SQL queries
        └── schema.rs             # Schema migrations
```

**Key insight:** The analyzer lives in `pioneer-test-ui/src/analyzer/`. The format library is pure I/O. The benchmark's Rust side needs a thin CLI binary that wraps `analyzer::analyze_track()` and outputs JSON.

---

## Ground Truth Strategy: master.db, Not PDB

The plan originally assumed we'd need to parse binary PDB/ANLZ from a Rekordbox-exported USB to extract ground truth BPM, key, and beats. The repo now has `reader/masterdb.rs` which changes this entirely.

### Why master.db is better

| | Export USB PDB/ANLZ | Local master.db |
|---|---|---|
| **Format** | Binary, custom DeviceSQL + custom ANLZ tags | SQLCipher SQLite, structured queries |
| **Parser needed** | Yes, ~400 lines new Rust or Python | **Already written** — `read_masterdb()` |
| **BPM** | Decode PDB track rows | `SELECT BPM FROM djmdContent` |
| **Key** | Decode PDB track rows + lookup table | `SELECT ScaleName FROM djmdKey JOIN djmdContent` |
| **Beat grids** | Parse ANLZ PQTZ binary tag | Not in master.db (see below) |
| **Cue points** | Parse ANLZ PCOB binary tag | `SELECT InMsec, OutMsec, Kind FROM djmdCue` |
| **Playlists** | Decode PDB playlist tree | `SELECT * FROM djmdSongPlaylist ORDER BY TrackNo` |
| **Artwork paths** | Not available | `SELECT ImagePath FROM djmdContent` |
| **File paths** | USB-relative → need reverse mapping | `SELECT FolderPath` (absolute local path) |

**BPM and key come from master.db for free.** No binary parsing needed for the two most important accuracy metrics.

### What master.db does NOT have

- **Beat grid positions** (individual beat timestamps) — Rekordbox stores these only in the ANLZ files on the exported USB, not in master.db
- **Waveform data** — same, only in ANLZ files

### Revised strategy

```
Ground truth source:
  ├── master.db → BPM, key, cue points, playlists, file paths     (already readable)
  └── ANLZ files → beat grid positions                              (need new reader)

ANLZ reader: mirror the existing anlz.rs writer in reverse.
Read PQTZ tag → extract beat timestamps.
This is the only new binary parsing needed.
```

This eliminates the need for a PDB reader entirely. The Python benchmark calls:
```bash
cargo run -p pioneer-usb-writer -- read-groundtruth /Volumes/REKORDBOX_USB --masterdb ~/Library/Pioneer/rekordbox/master.db --json
```

Or the Rust side produces a ground truth JSON that the Python benchmark consumes.

---

## Revision Notes After samplebase Harness Review

The samplebase harness has been updated on `feat/benchmark-harness-fixes` with three changes that affect our reuse strategy:

1. **Runs are now isolated** — each run gets its own vector store at `run_dir/vectors/`, not the global persistent store. Old runs with contaminated corpus are labeled "exploratory" in the UI. fourfour must use run-scoped semantics from day one.

2. **Source paths are preserved** — entries carry `source_path` + `prepared_path` separately. Results report `source_path` as canonical identity. fourfour must do the same.

3. **All backend pairings are analyzed** — not just the first. Progress is per-pairing.

### Safe to reuse from samplebase

- Benchmark directory layout pattern
- CLI command shape (`benchmark-init`, `benchmark-run`, `benchmark-analyze`)
- Backend registry / variant dict pattern
- Dataclass / typed-result style
- JSON artifact writing / loading helpers
- `search_unique_by_source()` vector dedup pattern (for Phase 5 embeddings)

### Do not reuse from samplebase

- Persistent vector store as benchmark corpus
- Subjective `gemini/local/tie` scoring model
- Lossy preprocessed audio as the analysis source
- Path-only cache keys
- Single-pairing analysis
- Dashboard orchestrator thread-per-backend pattern (fourfour uses simpler CLI execution)

---

## Architecture: What Gets Built

```
fourfour/
├── benchmark/                              ← NEW directory
│   ├── README.md
│   ├── schemas/                            ← JSON schemas for every artifact
│   │   ├── manifest.schema.json
│   │   ├── groundtruth.schema.json
│   │   ├── raw-analysis.schema.json
│   │   ├── comparisons.schema.json
│   │   └── analysis.schema.json
│   ├── manifests/                          ← track corpus manifests
│   │   └── corpus-v1.manifest.json
│   ├── groundtruth/                        ← Rekordbox reference data (from master.db + ANLZ)
│   │   └── corpus-v1.groundtruth.json
│   ├── results/{run_id}/                  ← run-scoped, immutable after completion
│   │   ├── config.json                     # backends, timings, corpus scope
│   │   ├── raw_analysis/
│   │   │   └── {backend_id}.json           # raw backend output per track
│   │   ├── comparisons.json                # objective comparison rows
│   │   ├── analysis.json                   # aggregate scorecard
│   │   └── audit_notes.json               # optional manual notes
│   └── cache/                              ← content/config-addressed analysis cache
│       └── analysis/{backend_id}/
│
└── analysis/                               ← NEW Python package
    ├── pyproject.toml
    ├── src/fourfour_analysis/
    │   ├── __init__.py
    │   ├── __main__.py
    │   ├── cli.py                          ← argparse: init, run, analyze, compare
    │   ├── config.py                       ← Settings dataclass, path resolution
    │   ├── types.py                        ← TrackEntry, AnalysisResult, AnalysisRecord,
    │   │                                     GroundTruth, TrackComparison, BackendMetadata
    │   ├── cache.py                        ← Content/config-addressed JSON cache
    │   ├── backends/
    │   │   ├── __init__.py
    │   │   ├── base.py                     ← AnalysisBackend ABC
    │   │   ├── registry.py                 ← ANALYSIS_VARIANTS dict
    │   │   ├── stratum_dsp.py              ← subprocess → Rust analyzer CLI
    │   │   └── (future: essentia.py, madmom.py, openkeyscan.py)
    │   ├── manifest.py                     ← Build/load track corpus manifest
    │   ├── groundtruth.py                  ← Load & validate ground truth JSON
    │   ├── compare.py                      ← Diff analysis vs ground truth
    │   ├── runner.py                       ← Run orchestration: analyze + compare + write
    │   └── analysis.py                     ← Aggregate metrics from comparisons
    └── tests/
        └── ...
```

---

## Rust-Side Changes

### Change 1: ANLZ reader (in `pioneer-usb-writer`)

Mirror the existing `writer/anlz.rs` in reverse. Read PQTZ tag to extract beat timestamps.

```rust
// pioneer-usb-writer/src/reader/anlz.rs (NEW)

pub struct AnlzBeatGrid {
    pub beats: Vec<AnlzBeat>,
}

pub struct AnlzBeat {
    pub time_ms: u32,
    pub bar_position: u8,
    pub tempo: u32,
}

/// Read beat grid from an ANLZ0000.DAT file.
pub fn read_beat_grid(path: &Path) -> Result<AnlzBeatGrid> {
    // Parse PMAI header, find PQTZ section, decode beat entries
    // Mirrors the write path in writer/anlz.rs
}
```

**Estimate:** ~150 lines. The format is already fully understood from the writer.

### Change 2: Ground truth extraction command

Add a thin binary (in `pioneer-test-ui` or a new `fourfour-bench` crate) that:
1. Reads `master.db` via `reader::masterdb::read_masterdb()` → gets BPM, key, cue points, file paths
2. Finds the matching ANLZ files on the Rekordbox-exported USB
3. Reads beat grids via `reader::anlz::read_beat_grid()`
4. Outputs a single JSON ground truth file

```bash
fourfour-bench extract-groundtruth \
  --masterdb ~/Library/Pioneer/rekordbox/master.db \
  --usb /Volumes/REKORDBOX_USB \
  --output benchmark/groundtruth/corpus-v1.groundtruth.json
```

Output schema:
```json
{
  "schema_version": 1,
  "source": {
    "masterdb_path": "~/Library/Pioneer/rekordbox/master.db",
    "usb_path": "/Volumes/REKORDBOX_USB"
  },
  "tracks": [
    {
      "track_id": "s_abc123",
      "file_path": "/Users/.../track.mp3",
      "title": "Track Name",
      "artist": "Artist",
      "genre": "House",
      "bpm": 128.02,
      "key": "1A",
      "beats": [0.461, 0.928, 1.395, "..."],
      "beat_count": 412,
      "cue_points": [
        {"hot_cue_number": 0, "time_ms": 461, "loop_time_ms": null}
      ],
      "rekordbox_content_id": "12345",
      "duration_seconds": 193.5
    }
  ]
}
```

### Change 3: Analyze single track command

Add a `analyze` subcommand to the `pioneer-test-ui` binary (or extract into a small `fourfour-analyzer` binary):

```bash
fourfour-analyzer analyze /path/to/track.mp3 --json
```

Output:
```json
{
  "path": "/path/to/track.mp3",
  "bpm": 128.0,
  "key": "1A",
  "beats": [0.461, 0.928, 1.395],
  "beat_count": 412,
  "waveform_preview": [34, 67, 23],
  "duration_seconds": 193.5,
  "sample_rate": 44100,
  "elapsed_seconds": 1.23,
  "version": "0.9.0"
}
```

Implementation: thin wrapper around `pioneer_test_ui::analyzer::analyze_track()`:

```rust
// Either in pioneer-test-ui/src/main.rs as a subcommand,
// or a new small binary crate.
fn cmd_analyze(path: &Path) -> Result<()> {
    let result = analyzer::analyze_track(path)?;
    let output = serde_json::json!({
        "path": path.to_str().unwrap(),
        "bpm": result.bpm,
        "key": result.key,
        "beats": result.beat_grid.beats.iter()
            .map(|b| b.time_ms as f64 / 1000.0).collect::<Vec<_>>(),
        "beat_count": result.beat_grid.beats.len(),
        "waveform_preview": result.waveform.data.to_vec(),
        "duration_seconds": 0.0, // TODO: from track metadata
        "elapsed_seconds": 0.0,  // filled by caller
        "version": pioneer_usb_writer::VERSION,
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
```

**Estimate:** ~40 lines of new code (CLI parsing) + the `analyze_track` function already exists.

### Where to put the CLI

**Option A: Subcommand in `pioneer-test-ui`** — smallest change, but couples the benchmark tool to the Tauri binary.

**Option B: New `fourfour-cli` crate** — clean separation. Depends on `pioneer-usb-writer` + copies `analyzer/` module from `pioneer-test-ui`. But duplicates the analyzer code.

**Option C: Extract `analyzer/` into a shared `fourfour-analyzer` library crate** — both `pioneer-test-ui` and `fourfour-cli` depend on it. Cleanest but needs workspace restructuring.

**Recommendation: Option A for now, Option C later.** The benchmark is experimental. Ship fast with a subcommand in `pioneer-test-ui`. Refactor into a shared crate once the analyzer stabilizes.

---

## Python Package: Build Steps

### Step 0: Lock artifact schemas

Before writing any logic, define JSON schemas:

| Schema | Purpose |
|---|---|
| `manifest.schema.json` | Track corpus: id, path, label, genre, duration_seconds, content_fingerprint |
| `groundtruth.schema.json` | Rekordbox reference: bpm, key, beats, cue_points, rekordbox_content_id |
| `raw-analysis.schema.json` | Per-backend per-track analysis output |
| `comparisons.schema.json` | Per-track comparison rows with status + metrics |
| `analysis.schema.json` | Aggregate scorecard per backend |

Each schema gets a version number. The runner validates inputs and outputs against schemas in tests. Cache keys include the output schema version.

### Step 1: Scaffold the Python package

```
analysis/
├── pyproject.toml         # pip install -e .
└── src/fourfour_analysis/
    ├── __init__.py
    ├── __main__.py         # python -m fourfour_analysis
    ├── cli.py              # argparse: init, run, analyze, compare
    └── config.py           # Settings dataclass
```

`config.py` — simplified from samplebase pattern:
```python
@dataclass(frozen=True)
class Settings:
    benchmark_dir: Path
    benchmark_cache_dir: Path
    benchmark_results_dir: Path
    benchmark_manifests_dir: Path
    groundtruth_dir: Path
    schemas_dir: Path
    # Rust binaries
    analyzer_bin: str       # e.g. "cargo run -p pioneer-test-ui -- analyze"
    bench_bin: str          # e.g. "cargo run -p pioneer-test-ui -- extract-groundtruth"
```

### Step 2: Build types and cache

`types.py` — core data types:
```python
@dataclass(frozen=True)
class TrackEntry:
    id: str
    path: str
    label: str
    genre: str
    duration_seconds: float
    content_fingerprint: str
    artist: str | None = None
    title: str | None = None

@dataclass(frozen=True)
class AnalysisResult:
    bpm: float | None
    key: str | None
    beats: list[float]            # timestamps in seconds
    waveform_preview: list[int]
    metrics: dict

@dataclass(frozen=True)
class AnalysisRecord:
    track_id: str
    backend_id: str
    status: str                   # ok | analysis_failed | unsupported | timeout
    result: AnalysisResult | None
    elapsed_seconds: float
    error: str | None
    backend_version: str
    backend_config_hash: str

@dataclass(frozen=True)
class GroundTruth:
    track_id: str
    bpm: float | None
    key: str | None
    beats: list[float]
    cue_points: list[dict]        # [{hot_cue_number, time_ms, loop_time_ms}]
    rekordbox_content_id: str | None
    file_path: str | None

@dataclass(frozen=True)
class TrackComparison:
    track_id: str
    track_label: str
    backend_id: str
    status: str                   # ok | missing_groundtruth | analysis_failed | ...
    bpm_ours: float | None
    bpm_groundtruth: float | None
    bpm_delta: float | None
    bpm_relative_delta_pct: float | None
    bpm_within_1_pct: bool | None
    bpm_octave_error: bool | None
    key_ours: str | None
    key_groundtruth: str | None
    key_exact_match: bool | None
    key_weighted_score: float | None
    key_error_type: str | None    # exact | relative_major_minor | parallel | fifth | other
    beats_count_ours: int
    beats_count_groundtruth: int
    beat_f_measure: float | None
    beat_offset_median_ms: float | None
    beat_offset_max_ms: float | None
    waveform_mse: float | None
```

`cache.py` — content/config-addressed caching:
```python
def cache_key(*, track: TrackEntry, backend: BackendMetadata, schema_version: int) -> str:
    payload = {
        "content_fingerprint": track.content_fingerprint,
        "backend_id": backend.id,
        "backend_version": backend.version,
        "backend_config_hash": backend.config_hash,
        "output_schema_version": schema_version,
    }
    return sha1(json.dumps(payload, sort_keys=True).encode()).hexdigest()[:24]
```

### Step 3: Build the AnalysisBackend ABC

```python
class AnalysisBackend(ABC):
    def __init__(self, *, settings: Settings, variant: dict):
        self.settings = settings
        self.variant = variant
        self.id = variant["id"]
        self.label = variant["label"]
        self.cache_dir = settings.benchmark_cache_dir / "analysis" / self.id

    @abstractmethod
    def metadata(self) -> dict: ...

    @abstractmethod
    def analyze_track(self, track: TrackEntry) -> AnalysisResult: ...

    def analyze_track_cached(self, track: TrackEntry) -> AnalysisRecord:
        """Analyze with caching. Returns AnalysisRecord with status."""
        cached = self._load_cache(track)
        if cached is not None:
            return cached
        started = time.perf_counter()
        try:
            result = self.analyze_track(track)
            record = AnalysisRecord(
                track_id=track.id, backend_id=self.id, status="ok",
                result=result, elapsed_seconds=time.perf_counter() - started,
                error=None, backend_version=self.metadata()["version"],
                backend_config_hash=self.metadata()["config_hash"],
            )
        except Exception as exc:
            record = AnalysisRecord(
                track_id=track.id, backend_id=self.id, status="analysis_failed",
                result=None, elapsed_seconds=time.perf_counter() - started,
                error=str(exc), backend_version=self.metadata()["version"],
                backend_config_hash=self.metadata()["config_hash"],
            )
        self._save_cache(track, record)
        return record
```

### Step 4: Implement StratumDspBackend

```python
class StratumDspBackend(AnalysisBackend):
    """Calls the Rust analyzer binary via subprocess."""

    def metadata(self) -> dict:
        return {
            "backend": "stratum_dsp",
            "language": "rust",
            "version": pioneer_usb_writer::VERSION,  # from JSON output
            "config_hash": "default",
            "network_required": False,
        }

    def analyze_track(self, track: TrackEntry) -> AnalysisResult:
        cmd = self.settings.analyzer_bin.split() + ["analyze", track.path, "--json"]
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
        if proc.returncode != 0:
            raise RuntimeError(f"Analyzer failed: {proc.stderr}")
        data = json.loads(proc.stdout)
        return AnalysisResult(
            bpm=data["bpm"],
            key=data["key"],
            beats=data["beats"],
            waveform_preview=data.get("waveform_preview", []),
            metrics={"sample_rate": data.get("sample_rate")},
        )
```

### Step 5: Build groundtruth.py

Loads the JSON produced by the Rust `extract-groundtruth` command. Validates against schema. Matches tracks to manifest entries by file path.

```python
def load_groundtruth(path: Path) -> dict[str, GroundTruth]:
    """Load ground truth JSON, keyed by track_id (matched by file_path)."""
    data = json.loads(path.read_text())
    by_path: dict[str, GroundTruth] = {}
    for t in data["tracks"]:
        gt = GroundTruth(
            track_id=t["track_id"],
            bpm=t["bpm"],
            key=t["key"],
            beats=t["beats"],
            cue_points=t.get("cue_points", []),
            rekordbox_content_id=t.get("rekordbox_content_id"),
            file_path=t.get("file_path"),
        )
        if gt.file_path:
            by_path[gt.file_path] = gt
    return by_path
```

### Step 6: Build compare.py

Objective diff logic — every manifest track produces a row.

```python
def compare_results(
    ours: dict[str, AnalysisRecord],
    ground_truth: dict[str, GroundTruth],
    manifest: list[TrackEntry],
    path_to_id: dict[str, str],
) -> list[TrackComparison]:
    comparisons = []
    for track in manifest:
        gt = ground_truth.get(path_to_id.get(track.path, ""))
        record = ours.get(track.id)

        if gt is None:
            comparisons.append(_missing_groundtruth_row(track, record))
            continue
        if record is None:
            comparisons.append(_missing_analysis_row(track, gt))
            continue
        if record.status != "ok" or record.result is None:
            comparisons.append(_failed_analysis_row(track, gt, record))
            continue

        result = record.result
        tempo = compare_tempo(result.bpm, gt.bpm)
        key = compare_key(result.key, gt.key)
        beats = compare_beats(result.beats, gt.beats)

        comparisons.append(TrackComparison(
            track_id=track.id, track_label=track.label,
            backend_id=record.backend_id, status="ok",
            bpm_ours=result.bpm, bpm_groundtruth=gt.bpm,
            bpm_delta=tempo.abs_delta, bpm_relative_delta_pct=tempo.relative_pct,
            bpm_within_1_pct=tempo.within_1_pct, bpm_octave_error=tempo.octave_error,
            key_ours=result.key, key_groundtruth=gt.key,
            key_exact_match=key.exact, key_weighted_score=key.weighted_score,
            key_error_type=key.error_type,
            beats_count_ours=len(result.beats),
            beats_count_groundtruth=len(gt.beats),
            beat_f_measure=beats.f_measure,
            beat_offset_median_ms=beats.median_offset_ms,
            beat_offset_max_ms=beats.max_offset_ms,
            waveform_mse=None,
        ))
    return comparisons
```

**Beat comparison** uses standard MIR metrics (not just nearest-neighbor offset):
- **F-measure** with tolerance window (e.g. ±50ms): precision × recall
- **Cemgil score**: Gaussian-weighted match score
- **Continuity**: longest consecutive correctly-tracked segment
- Median/max offset as supplementary interpretable metrics
- Implementation: either `mir_eval.beat` (preferred) or minimal port with golden tests

**Key comparison** categorizes errors:
- Exact match → `1.0`
- Relative major/minor (1A ↔ 1B) → `0.8`, error_type = `relative_major_minor`
- Parallel major/minor → `0.6`
- Fifth apart → `0.4`
- Other → `0.0`

**Tempo comparison** detects octave errors:
- Absolute delta, relative delta percentage
- `within_1_pct`, `within_4_pct`
- Octave error: one is ~2× or ~0.5× the other

### Step 7: Build runner.py

Run-scoped orchestration. No persistent vector store.

```
1. Load manifest (track list)
2. Load ground truth (from master.db + ANLZ extraction)
3. For each backend variant:
   a. Instantiate backend
   b. For each track: backend.analyze_track_cached(track) → AnalysisRecord
   c. Write raw_analysis/{backend_id}.json
   d. Compare all records to ground truth → list[TrackComparison]
4. Write config.json, comparisons.json, analysis.json
5. Print summary table with coverage counts
```

### Step 8: Build analysis.py

Aggregates comparisons into a scorecard:

```json
{
  "run_id": "run-20260417T120000Z",
  "generated_at": "...",
  "backends": {
    "stratum_dsp_default": {
      "manifest_tracks": 30,
      "tracks_compared": 28,
      "missing_groundtruth": 1,
      "analysis_failures": 1,
      "bpm": {
        "median_abs_delta": 0.15,
        "within_1_pct": 0.96,
        "within_4_pct": 1.0,
        "octave_error_rate": 0.03,
        "missing_count": 0
      },
      "key": {
        "exact_match_rate": 0.80,
        "weighted_score_mean": 0.86,
        "error_breakdown": {
          "relative_major_minor": 3,
          "parallel": 1,
          "fifth": 2,
          "other": 0
        },
        "missing_count": 1
      },
      "beats": {
        "f_measure_mean": 0.94,
        "f_measure_median": 0.97,
        "median_offset_ms": 2.3,
        "count_delta_median": 1.0
      },
      "operational": {
        "avg_analysis_time_seconds": 1.2,
        "p95_analysis_time_seconds": 2.7,
        "cache_hit_rate": 0.0,
        "failures": 0
      }
    }
  },
  "recommendation": "stratum_dsp_default meets compatibility targets for BPM and beat grid; key accuracy needs manual review."
}
```

---

## Build Order

```
Phase R (Rust-side, ~1 day):
  ┌─────────────────────────────────────────┐
  │ R1. ANLZ reader in pioneer-usb-writer   │  ~150 lines, mirrors anlz.rs writer
  │ R2. extract-groundtruth command          │  ~80 lines, uses read_masterdb + ANLZ reader
  │ R3. analyze --json command               │  ~40 lines, wraps analyzer::analyze_track
  └─────────────────────────────────────────┘

Phase P (Python-side, ~3-4 days):
  ┌─────────────────────────────────────────┐
  │ P0. JSON schemas for all artifacts       │  ~150 lines
  │ P1. Scaffold package + config            │  ~100 lines
  │ P2. Types + cache                        │  ~200 lines
  │ P3. AnalysisBackend ABC + StratumDsp     │  ~150 lines
  │ P4. groundtruth.py (loader)              │  ~60 lines
  │ P5. compare.py (diff logic)              │  ~200 lines
  │ P6. runner.py (orchestration)            │  ~200 lines
  │ P7. analysis.py (aggregation)            │  ~100 lines
  │ P8. cli.py (argparse)                    │  ~100 lines
  └─────────────────────────────────────────┘

Phase T (Testing, ~1-2 days):
  ┌─────────────────────────────────────────┐
  │ T1. Curate test corpus (30 tracks)       │
  │ T2. Export from Rekordbox → USB          │
  │ T3. Run extract-groundtruth              │
  │ T4. Run first benchmark                  │
  │ T5. Review results, iterate              │
  └─────────────────────────────────────────┘
```

---

## Files: New vs Modified

### Rust-side (new code)
| File | Purpose | Est. Lines |
|---|---|---|
| `pioneer-usb-writer/src/reader/anlz.rs` | ANLZ PQTZ beat grid reader | ~150 |
| `pioneer-test-ui/src/main.rs` | Add `analyze` and `extract-groundtruth` subcommands | ~120 |

### Python-side (all new)
| File | Purpose | Est. Lines |
|---|---|---|
| `analysis/src/fourfour_analysis/types.py` | Core data types | ~100 |
| `analysis/src/fourfour_analysis/cache.py` | Content-addressed JSON cache | ~80 |
| `analysis/src/fourfour_analysis/backends/base.py` | AnalysisBackend ABC | ~60 |
| `analysis/src/fourfour_analysis/backends/stratum_dsp.py` | Subprocess wrapper | ~80 |
| `analysis/src/fourfour_analysis/backends/registry.py` | ANALYSIS_VARIANTS dict | ~30 |
| `analysis/src/fourfour_analysis/manifest.py` | Corpus manifest builder | ~80 |
| `analysis/src/fourfour_analysis/groundtruth.py` | Ground truth loader | ~60 |
| `analysis/src/fourfour_analysis/compare.py` | Diff logic + metrics | ~200 |
| `analysis/src/fourfour_analysis/runner.py` | Run orchestration | ~200 |
| `analysis/src/fourfour_analysis/analysis.py` | Aggregation | ~100 |
| `analysis/src/fourfour_analysis/cli.py` | argparse CLI | ~100 |
| `analysis/src/fourfour_analysis/config.py` | Settings | ~40 |
| `benchmark/schemas/*.schema.json` | Artifact validation | ~150 |
| `analysis/pyproject.toml` | Package config | ~15 |

**Total new code: ~1,545 lines** (Rust ~270, Python ~1,130, schemas ~150)

---

## CLI Interface

```bash
# Install
cd fourfour/analysis
python3 -m venv .venv && source .venv/bin/activate
pip install -e .

# Step 1: Create test corpus manifest from a directory of tracks
fourfour-benchmark init ~/Music/benchmark-corpus --name corpus-v1

# Step 2: Extract ground truth (Rust-side, reads master.db + USB ANLZ)
cargo run -p pioneer-test-ui -- extract-groundtruth \
  --masterdb ~/Library/Pioneer/rekordbox/master.db \
  --usb /Volumes/REKORDBOX_USB \
  --manifest benchmark/manifests/corpus-v1.manifest.json \
  --output benchmark/groundtruth/corpus-v1.groundtruth.json

# Step 3: Run benchmark
fourfour-benchmark run \
  --manifest benchmark/manifests/corpus-v1.manifest.json \
  --groundtruth benchmark/groundtruth/corpus-v1.groundtruth.json \
  --variants stratum_dsp_default

# Step 4: View results
fourfour-benchmark analyze run-20260417T120000Z

# Future: Add more backends
fourfour-benchmark run \
  --manifest benchmark/manifests/corpus-v1.manifest.json \
  --groundtruth benchmark/groundtruth/corpus-v1.groundtruth.json \
  --variants stratum_dsp_default essentia_tempocnn madmom_dbn
```

---

## Test Corpus Requirements

For Phase 0, ~30 tracks with known Rekordbox analysis:

| Genre | Count | BPM Range | Why |
|---|---|---|---|
| House/Techno | 10 | 120-135 | Steady 4/4, tests basic accuracy |
| Drum & Bass | 5 | 170-175 | Half-time detection, octave errors |
| Hip-Hop | 5 | 80-100 | Variable tempo, swung beats |
| Broken beat / halftime | 5 | mixed | Odd meters, tempo changes |
| Pop/Rock | 5 | 100-140 | Live drums, tempo drift |

**Process:**
1. Curate these tracks in a Rekordbox collection
2. Let Rekordbox analyze them (full analysis with beat grids)
3. Export to USB (creates ANLZ files with beat grids)
4. Copy the same source files to our benchmark corpus directory
5. Run `extract-groundtruth` to pull BPM/key from master.db + beats from ANLZ
6. Run `fourfour-benchmark init` on the corpus directory
7. Run `fourfour-benchmark run` to compare our analysis vs Rekordbox
