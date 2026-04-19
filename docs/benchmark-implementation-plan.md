# fourfour Benchmark — Implementation Plan

> Concrete plan to fork the samplebase benchmark harness and build a ground-truth comparison tool for Pioneer USB analysis quality.

---

## Architecture: What Gets Forked vs Built New

```
fourfour/
├── benchmark/                          ← NEW directory (mirrors samplebase/benchmark/)
│   ├── README.md
│   ├── manifests/                      ← same pattern as samplebase
│   │   └── corpus-v1.manifest.json
│   ├── groundtruth/                    ← NEW: Rekordbox reference data
│   │   └── corpus-v1.groundtruth.json
│   ├── results/{run_id}/              ← same structure as samplebase
│   │   ├── config.json
│   │   ├── comparisons.json            ← NEW (replaces results.json)
│   │   ├── scores.json
│   │   └── analysis.json
│   └── cache/                          ← same SHA1 caching as samplebase
│       └── embeddings/{backend_id}/
│
└── analysis/                           ← NEW Python package
    ├── pyproject.toml
    ├── src/fourfour_analysis/
    │   ├── __init__.py
    │   ├── __main__.py                 ← CLI entry point
    │   ├── cli.py                      ← argparse: init, run, analyze, compare
    │   │
    │   ├── config.py                   ← FORKED from samplebase/config.py (simplified)
    │   ├── types.py                    ← FORKED from samplebase/benchmark_types.py (adapted)
    │   │
    │   ├── backends/                   ← FORKED pattern from samplebase/benchmark_backends.py
    │   │   ├── __init__.py
    │   │   ├── base.py                 ← ABC: AnalysisBackend (adapted from BenchmarkBackend)
    │   │   ├── registry.py             ← FORKED pattern: ANALYSIS_VARIANTS dict
    │   │   ├── stratum_dsp.py          ← NEW: subprocess wrapper around Rust CLI
    │   │   └── (future: essentia.py, madmom.py, openkeyscan.py)
    │   │
    │   ├── cache.py                    ← FORKED: SHA1-keyed JSON cache (verbatim copy)
    │   │
    │   ├── groundtruth.py              ← NEW: Rekordbox PDB + ANLZ parser
    │   ├── manifest.py                 ← FORKED from samplebase/benchmark_manifest.py (simplified)
    │   ├── runner.py                   ← FORKED from samplebase/benchmark_runner.py (adapted)
    │   ├── analysis.py                 ← FORKED from samplebase/benchmark_analysis.py (adapted)
    │   │
    │   └── compare.py                  ← NEW: diff logic (ours vs Rekordbox ground truth)
    │
    └── tests/
        └── ...
```

---

## Step-by-Step Build Order

### Step 1: Scaffold the Python package

Create `analysis/` as an installable Python package with a CLI.

**Files to create:**
- `analysis/pyproject.toml` — minimal, depends on: `anyhow` pattern is Python so just `anyhow` equivalent = nothing special. Deps: `numpy` (future), no external deps yet
- `analysis/src/fourfour_analysis/__init__.py`
- `analysis/src/fourfour_analysis/__main__.py` — `python -m fourfour_analysis` entry
- `analysis/src/fourfour_analysis/cli.py` — argparse with subcommands: `init`, `run`, `analyze`, `compare`
- `analysis/src/fourfour_analysis/config.py` — FORKED from samplebase, stripped to just benchmark paths

**Config changes from samplebase:**
```python
# samplebase config has: db_path, gemini_api_key, describe_model, embedding_model,
#   output_dimensionality, upload_sample_rate_hz, upload_bitrate_kbps, etc.
# fourfour config only needs:
@dataclass(frozen=True)
class Settings:
    benchmark_dir: Path
    benchmark_cache_dir: Path
    benchmark_results_dir: Path
    benchmark_manifests_dir: Path
    groundtruth_dir: Path
    # Rust binary path (for stratum-dsp backend)
    pioneer_usb_writer_bin: str  # defaults to "cargo run -p pioneer-usb-writer --"
```

### Step 2: Fork the caching layer (verbatim)

Copy `samplebase/mvp/src/samplebase_mvp/benchmark_backends.py` lines related to caching into `cache.py`.

**What to extract:**
- `_cache_key(self, *, path, segment) -> str` — SHA1 hash
- `_load_cached_embedding(self, *, path, segment) -> EmbeddingVector | None`
- `_save_cached_embedding(self, *, path, segment, embedding)`

**Changes:**
- Rename `EmbeddingVector` → `CachedAnalysis` (or keep as generic `CachedResult`)
- `segment` parameter becomes unnecessary for fourfour (we always analyze full tracks), replace with just `track_id: str`
- Cache key: `sha1("{path}|{backend_id}")`

This is ~60 lines of code, nearly verbatim.

### Step 3: Fork the types (adapted)

From `samplebase/mvp/src/samplebase_mvp/benchmark_types.py`:

**Keep (rename):**
- `EmbeddingVector` → `CachedResult` — `{values: list[float], elapsed_seconds: float, metrics: dict}`

**New types for fourfour:**
```python
@dataclass(frozen=True)
class TrackEntry:
    id: str
    path: str
    label: str           # filename or title
    genre: str
    duration_seconds: float

@dataclass(frozen=True)
class AnalysisResult:
    bpm: float
    key: str             # "1A", "5B", etc.
    beats: list[float]   # timestamps in seconds
    waveform_preview: list[int]  # 400 bytes as int list (JSON-safe)
    elapsed_seconds: float
    metrics: dict

@dataclass(frozen=True)
class GroundTruth:
    track_id: str
    bpm: float
    key: str
    beats: list[float]
    # waveform: list[int]  # future: compare waveforms too

@dataclass(frozen=True)
class TrackComparison:
    track_id: str
    track_label: str
    bpm_ours: float
    bpm_groundtruth: float
    bpm_delta: float
    key_ours: str
    key_groundtruth: str
    key_match: bool
    beats_count_ours: int
    beats_count_groundtruth: int
    beat_offset_mean_ms: float | None
    beat_offset_max_ms: float | None
    waveform_mse: float | None
```

### Step 4: Build the AnalysisBackend ABC

From `samplebase/mvp/src/samplebase_mvp/benchmark_backends.py` — extract `BenchmarkBackend` class.

**samplebase's interface:**
```python
class BenchmarkBackend(ABC):
    def embed_audio_segments(self, item: dict) -> CorpusEmbedding
    def embed_text_query(self, query: dict) -> QueryEmbedding
    def score(self, query_vectors, corpus_vectors) -> float
    # + caching helpers
```

**fourfour's interface:**
```python
class AnalysisBackend(ABC):
    """Base class for track analysis backends."""
    
    def __init__(self, *, settings: Settings, variant: dict):
        self.settings = settings
        self.variant = variant
        self.id = variant["id"]
        self.label = variant["label"]
        self.cache_dir = settings.benchmark_cache_dir / "analysis" / self.id
        self.cache_dir.mkdir(parents=True, exist_ok=True)
    
    @abstractmethod
    def metadata(self) -> dict:
        """Backend info for config.json."""
        ...
    
    @abstractmethod
    def analyze_track(self, track_path: str) -> AnalysisResult:
        """Analyze a single track. Must be overridden by each backend."""
        ...
    
    def analyze_track_cached(self, track: TrackEntry) -> AnalysisResult:
        """Analyze with caching. Subclasses should NOT override this."""
        cached = self._load_cache(track.id)
        if cached is not None:
            return cached
        
        started = time.perf_counter()
        result = self.analyze_track(track.path)
        result.elapsed_seconds = time.perf_counter() - started
        self._save_cache(track.id, result)
        return result
    
    # Cache helpers (forked from samplebase)
    def _cache_key(self, track_id: str) -> str: ...
    def _load_cache(self, track_id: str) -> AnalysisResult | None: ...
    def _save_cache(self, track_id: str, result: AnalysisResult) -> None: ...
```

### Step 5: Implement StratumDspBackend

The first concrete backend. Calls the Rust binary via subprocess.

```python
class StratumDspBackend(AnalysisBackend):
    """Calls pioneer-usb-writer Rust binary via subprocess."""
    
    def metadata(self) -> dict:
        return {
            "backend": "stratum_dsp",
            "language": "rust",
            "chunking_policy": "full",
            "network_required": False,
        }
    
    def analyze_track(self, track_path: str) -> AnalysisResult:
        # Option A: Add a --analyze --json subcommand to the Rust CLI
        # Option B: Use the existing library via a thin Rust binary that outputs JSON
        #
        # For now, we need to ADD a JSON output mode to the Rust CLI.
        # The current main.rs only does scan→analyze→write_usb in one shot.
        # We need a separate mode that outputs analysis for a single track as JSON.
        ...
```

**IMPORTANT: This requires a Rust-side change first.** The current `pioneer-usb-writer` CLI doesn't have an "analyze single track and output JSON" mode. We need to add one.

### Step 5a: Add `analyze` subcommand to Rust CLI

Modify `pioneer-usb-writer/src/main.rs` to support:

```bash
# Current behavior (unchanged):
pioneer-usb-writer /path/to/music -o /Volumes/USB

# New behavior:
pioneer-usb-writer analyze /path/to/track.mp3 --json
```

Output:
```json
{
  "path": "/path/to/track.mp3",
  "bpm": 128.0,
  "key": "1A",
  "beats": [0.461, 0.928, 1.395, ...],
  "beat_count": 412,
  "waveform_preview": [34, 67, 23, ...],
  "duration_seconds": 193.5,
  "sample_rate": 44100
}
```

This is a small Rust change (~30 lines in `main.rs`):
```rust
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    
    // Legacy: if no subcommand, run the original scan→write flow
    input_dirs: Vec<PathBuf>,
    #[arg(short, long)]
    output: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze a single track and output results as JSON
    Analyze {
        /// Path to audio file
        path: PathBuf,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}
```

### Step 6: Fork the manifest builder (simplified)

From `samplebase/mvp/src/samplebase_mvp/benchmark_manifest.py`.

**samplebase's manifest has per-entry:**
`id, path, label, category, role, one_shot_or_loop, duration_bucket, duration_seconds, tags, known_similar_group, bleed_risk_group, is_long_form`

**fourfour's manifest needs per-entry:**
`id, path, label, genre, duration_seconds`

Way simpler. Strip out all the sample-specific inference (`infer_role`, `infer_loop_state`, `infer_tags`, `infer_group`, `infer_bleed_group`). Keep:
- `build_manifest()` — scans directory for audio files
- `load_json()` — reads JSON
- `write_manifest_bundle()` — writes manifest + ground truth to benchmark dir

New additions:
- Audio file detection can use the same `SUPPORTED_SUFFIXES` pattern from samplebase
- Duration probing uses `ffprobe` (same as samplebase) or we could use `mutagen`/`tinytag` for a lighter dep

### Step 7: Build the ground truth parser (NEW)

This is the main new code. Two parsers needed:

**`groundtruth.py` — Rekordbox export parser:**

```python
def parse_rekordbox_usb(usb_path: Path) -> list[GroundTruth]:
    """Parse a Rekordbox-exported USB to extract ground truth data.
    
    Reads:
      - PIONEER/rekordbox/export.pdb  → BPM, key, track file paths
      - PIONEER/USBANLZ/P*/ANLZ0000.DAT → beat positions
    """
    ...
```

Implementation strategy:
1. Parse `export.pdb` binary format — we already have a *writer* in Rust (`pdb.rs`). The parser reads the same format. Can port the read path from the Rust reference code in `pioneer-usb-writer/reference-code/`, or write a fresh Python parser that reads just the track rows we need (BPM, key, file path).
2. Parse ANLZ files for beat grids — same story, we have the writer in `anlz.rs`, need a reader.

**Alternative shortcut:** Instead of parsing binary PDB/ANLZ from Python, we could:
- Add a `parse-usb` subcommand to the Rust CLI that reads a Rekordbox USB and outputs JSON
- Python benchmark calls `cargo run -p pioneer-usb-writer -- parse-usb /Volumes/REKORDBOX_USB --json`
- This reuses the existing Rust knowledge of the binary format

**Recommendation: Rust-side parser.** It's much easier to read a binary format you already know how to write. The Rust binary already links `symphonia`, `lofty`, etc. Adding a PDB reader is ~200 lines of Rust that mirrors the writer's structure.

### Step 8: Build the comparison logic (NEW)

**`compare.py` — diff our output vs ground truth:**

```python
def compare_results(
    ours: dict[str, AnalysisResult],    # keyed by track_id
    ground_truth: dict[str, GroundTruth], # keyed by track_id
) -> list[TrackComparison]:
    """Compare analysis results against ground truth."""
    comparisons = []
    for track_id, gt in ground_truth.items():
        our = ours.get(track_id)
        if our is None:
            continue  # track not in our corpus
        
        # Match beats by finding closest pairs
        beat_offsets = _align_beats(our.beats, gt.beats)
        
        comparisons.append(TrackComparison(
            track_id=track_id,
            track_label=gt.label,
            bpm_ours=our.bpm,
            bpm_groundtruth=gt.bpm,
            bpm_delta=abs(our.bpm - gt.bpm),
            key_ours=our.key,
            key_groundtruth=gt.key,
            key_match=_keys_equivalent(our.key, gt.key),
            beats_count_ours=len(our.beats),
            beats_count_groundtruth=len(gt.beats),
            beat_offset_mean_ms=_mean(beat_offsets) if beat_offsets else None,
            beat_offset_max_ms=_max(beat_offsets) if beat_offsets else None,
            waveform_mse=None,  # future
        ))
    return comparisons

def _align_beats(ours: list[float], theirs: list[float]) -> list[float]:
    """Find closest beat pairs and return offset in ms."""
    offsets = []
    j = 0
    for our_t in ours:
        # Find closest beat in theirs
        while j < len(theirs) - 1 and abs(theirs[j+1] - our_t) < abs(theirs[j] - our_t):
            j += 1
        offsets.append(abs(our_t - theirs[j]) * 1000)  # ms
    return offsets

def _keys_equivalent(a: str, b: str) -> bool:
    """Check if two key notations are equivalent.
    
    Handles: "1A" == "A♭m", "5B" == "E major", etc.
    Simple version: exact string match on Camelot notation.
    """
    return a.strip().upper() == b.strip().upper()
```

### Step 9: Fork the runner (adapted)

From `samplebase/mvp/src/samplebase_mvp/benchmark_runner.py`.

**What to keep:**
- `run_benchmark()` orchestration pattern
- Per-run directory creation
- `config.json` / `analysis.json` writing
- Backend instantiation + parallel execution (thread-per-backend)
- `load_run_detail()` for re-loading results

**What to change:**
- Replace `_embed_corpus()` → `_analyze_corpus()` — calls `backend.analyze_track_cached()` for each entry
- Replace `_score_query()` → `_compare_to_groundtruth()` — uses `compare.py`
- Result structure: `comparisons.json` instead of `results.json`

**Runner flow:**
```
1. Load manifest (track list)
2. Load ground truth (Rekordbox reference)
3. For each backend variant:
   a. Instantiate backend
   b. For each track in manifest:
      - backend.analyze_track_cached(track) → AnalysisResult
   c. Compare all results to ground truth → list[TrackComparison]
4. Write config.json, comparisons.json, analysis.json
5. Print summary table
```

### Step 10: Fork the analysis module (adapted)

From `samplebase/mvp/src/samplebase_mvp/benchmark_analysis.py`.

**What to keep:**
- `compute_analysis()` pattern
- `save_analysis()` / `load_scores()`
- Per-backend quality bucket pattern

**What to change:**
- Instead of `gemini_wins` / `local_wins` → track per-backend: `bpm_accuracy`, `key_accuracy`, `mean_beat_offset_ms`
- Aggregation: average across all tracks, grouped by genre

**New analysis output:**
```json
{
  "run_id": "run-20260417T120000Z",
  "backends": {
    "stratum_dsp_default": {
      "tracks_compared": 30,
      "bpm": {
        "mean_delta": 0.15,
        "within_0.5_pct": 0.93,
        "octave_errors": 1,
        "complete_misses": 0
      },
      "key": {
        "exact_match_rate": 0.80,
        "relative_major_minor_confusions": 3,
        "complete_misses": 1
      },
      "beats": {
        "mean_offset_ms": 2.3,
        "max_offset_ms": 8.1,
        "count_delta_mean": 1.2
      },
      "operational": {
        "avg_analysis_time_seconds": 1.2,
        "cache_hit_rate": 0.0,
        "failures": 0
      }
    }
  },
  "recommendation": "stratum_dsp_default meets accuracy targets (BPM ≥95%, Key ≥85%)"
}
```

---

## Build Order (Dependency Chain)

```
Weekend 1 (Day 1-2):
  ┌─────────────────────────────────────┐
  │ 1. Scaffold Python package          │  ← no deps, pure boilerplate
  │ 2. Fork config.py (simplified)      │
  │ 3. Fork cache.py (verbatim)         │
  │ 4. Fork types.py (adapted)          │
  └──────────────┬──────────────────────┘
                 │
  ┌──────────────▼──────────────────────┐
  │ 5a. Add `analyze --json` to Rust    │  ← ~30 lines in main.rs
  │     CLI                              │
  └──────────────┬──────────────────────┘
                 │
  ┌──────────────▼──────────────────────┐
  │ 4. Build AnalysisBackend ABC        │  ← fork pattern from samplebase
  │ 5. Implement StratumDspBackend      │  ← subprocess wrapper
  └──────────────┬──────────────────────┘
                 │
Weekend 1 (Day 2-3):
  ┌──────────────▼──────────────────────┐
  │ 6. Fork manifest.py (simplified)    │  ← strip sample-specific stuff
  │ 7. Build groundtruth.py             │  ← NEW: PDB/ANLZ parser
  │    (or Rust parse-usb subcommand)   │
  └──────────────┬──────────────────────┘
                 │
  ┌──────────────▼──────────────────────┐
  │ 8. Build compare.py                 │  ← NEW: diff logic
  └──────────────┬──────────────────────┘
                 │
Weekend 2:
  ┌──────────────▼──────────────────────┐
  │ 9. Fork runner.py (adapted)         │  ← main orchestration
  │ 10. Fork analysis.py (adapted)      │  ← aggregation/scoring
  │ 11. Wire up CLI                     │
  └──────────────┬──────────────────────┘
                 │
  ┌──────────────▼──────────────────────┐
  │ 12. Curate test corpus (30 tracks)  │
  │ 13. Export from Rekordbox → USB     │
  │ 14. Run first benchmark!            │
  └─────────────────────────────────────┘
```

---

## Files: Fork vs New vs Modify

### Forked from samplebase (copy + adapt):
| Source | Destination | Changes |
|---|---|---|
| `benchmark_backends.py` (cache helpers, ~60 lines) | `cache.py` | Rename types, simplify key (no segment) |
| `benchmark_backends.py` (ABC structure) | `backends/base.py` | New interface: `analyze_track()` instead of `embed_audio_segments()` |
| `benchmark_runner.py` (DEFAULT_VARIANTS, run orchestration, result writing) | `runner.py` | Swap embed→analyze, score→compare |
| `benchmark_analysis.py` (compute_analysis, save/load) | `analysis.py` | New metrics (accuracy instead of win-rate) |
| `benchmark_manifest.py` (build_manifest, load_json, write_bundle) | `manifest.py` | Strip sample-specific inference functions |
| `benchmark_types.py` (dataclass patterns) | `types.py` | New types for analysis results |
| `config.py` (Settings dataclass, path resolution) | `config.py` | Strip Gemini/embedding/sample-specific fields |

### New code:
| File | Purpose | Est. Lines |
|---|---|---|
| `groundtruth.py` | Parse Rekordbox USB export (PDB + ANLZ) | ~200 |
| `compare.py` | Diff analysis results vs ground truth | ~120 |
| `backends/stratum_dsp.py` | Subprocess wrapper for Rust CLI | ~80 |
| `cli.py` | argparse: init, run, analyze, compare | ~100 |

### Rust-side modification:
| File | Change | Est. Lines |
|---|---|---|
| `pioneer-usb-writer/src/main.rs` | Add `analyze` subcommand with `--json` output | ~40 |
| (optional) `pioneer-usb-writer/src/main.rs` | Add `parse-usb` subcommand for ground truth extraction | ~200 |

---

## CLI Interface

```bash
# Install
cd fourfour/analysis
python3 -m venv .venv && source .venv/bin/activate
pip install -e .

# Step 1: Create a test corpus manifest from a directory of tracks
fourfour-benchmark init ~/Music/benchmark-corpus --name corpus-v1

# Step 2: Extract ground truth from a Rekordbox-exported USB
fourfour-benchmark extract-groundtruth /Volumes/REKORDBOX_USB --name corpus-v1
# (or: cargo run -p pioneer-usb-writer -- parse-usb /Volumes/REKORDBOX_USB --json > groundtruth.json)

# Step 3: Run all backends against the corpus
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

## Decision: Rust-side or Python-side ground truth parser?

**Option A: Python PDB/ANLZ parser (~200 lines)**
- Pros: No Rust changes needed for ground truth extraction, Python is easier to iterate on
- Cons: Duplicates binary format knowledge that already exists in Rust writer, two sources of truth for format

**Option B: Rust `parse-usb` subcommand (~200 lines in Rust)**
- Pros: Single source of truth for PDB/ANLZ format, Rust already has the writer = reader is trivial mirror
- Cons: More Rust code to maintain, Python can't run without compiling Rust first

**Recommendation: Option B (Rust parser).** The PDB writer is ~920 lines. Writing a reader in Python that correctly handles all the binary edge cases would be error-prone. The Rust binary already knows every page offset and string encoding detail. A reader is just the writer in reverse.

The ground truth extraction becomes:
```bash
cargo run -p pioneer-usb-writer -- parse-usb /Volumes/REKORDBOX_USB --json
```
Output is JSON that Python can consume directly.

---

## Test Corpus Requirements

For Phase 0, we need ~30 tracks with known Rekordbox analysis:

| Genre | Count | BPM Range | Why |
|---|---|---|---|
| House/Techno | 10 | 120-135 | Steady 4/4, tests basic accuracy |
| Drum & Bass | 5 | 170-175 | Half-time detection, octave errors |
| Hip-Hop | 5 | 80-100 | Variable tempo, swung beats |
| Broken beat / halftime | 5 | mixed | Odd meters, tempo changes |
| Pop/Rock | 5 | 100-140 | Live drums, tempo drift |

**Process:**
1. Curate these tracks in a Rekordbox collection
2. Export to USB with full analysis (right-click → Export)
3. Also copy the same files to our benchmark corpus directory
4. Run `fourfour-benchmark extract-groundtruth` to parse the USB
5. Run `fourfour-benchmark init` on the corpus directory
6. Run `fourfour-benchmark run` to compare our analysis vs Rekordbox
