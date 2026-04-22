# fourfour — Experimentation Phase Path

> Goal: Validate every analysis layer against real hardware (CDJ-3000) and decide what goes into the production stack. The current MVP proves the *write path* works — this document plans the *analysis quality* journey.

---

## Where We Are Now (MVP Baseline)

| Layer | Current Implementation | Status |
|---|---|---|
| **Audio I/O** | `symphonia` (Rust, pure) | ✅ MP3, FLAC, WAV, AIFF, AAC |
| **Metadata** | `lofty` (Rust) | ✅ Tags, artwork, properties |
| **BPM / Key** | `stratum-dsp` (Rust) | ⚠️ Works, accuracy unknown vs hardware |
| **Beat Grid** | `stratum-dsp` beat positions → Pioneer format | ⚠️ Grid plays on CDJ, precision untested |
| **Waveform** | RMS-based monochrome 400-byte preview | ⚠️ Faked green color bands (PWV3/4/5) |
| **PDB Database** | Custom Rust writer, single-page | ✅ Reads on CDJ-3000 |
| **ANLZ Files** | Custom Rust writer, correct path hash | ✅ Reads on CDJ-3000 |
| **Artwork** | `image` crate, 80×80 + 240×240 resize | ✅ Displays on CDJ |
| **Test UI** | Tauri v2, vanilla HTML/JS | ✅ Functional throwaway |

### Known Gaps
1. **Color waveforms** — PWV3/PWV4/PWV5 are hardcoded green, no spectral coloring
2. **PDB multi-page** — Single 4096-byte page per table; breaks at ~10 tracks
3. **BPM accuracy** — No benchmark against Rekordbox ground truth
4. **Key accuracy** — No benchmark; `stratum-dsp` key detection quality unknown
5. **No phrase/structure analysis** — Rekordbox auto-phrases have no equivalent
6. **No audio similarity** — No embeddings, no smart crate digging
7. **No stems** — No source separation

---

## Existing Asset: samplebase Benchmark Harness

Benchmarking of audio analysis libraries was completed externally in the **samplebase** project (`~/dev/projects/samplebase`). The results and concrete recommendations are captured in [`analysis-pipeline-handoff.md`](./analysis-pipeline-handoff.md). Key findings:

- **BPM:** DeepRhythm is best (~97% Acc2 accuracy). stratum-dsp accuracy still untested vs Rekordbox.
- **Key:** librosa chroma_cqt + Krumhansl-Schmuckler (~70%). OpenKeyScan is the upgrade path if needed (~85-90%).
- **Energy:** librosa feature fusion, validated on 19 genres.
- **Embeddings:** MS CLAP best for text+audio, LAION-CLAP best for audio→audio.

### What samplebase built
A full-stack benchmark framework for comparing audio embedding backends (Gemini, LAION-CLAP, MS-CLAP, spectral baseline). The relevant architecture:

```
samplebase/mvp/src/samplebase_mvp/
├── benchmark_backends.py      # Pluggable backend interface (abstract class + 4 implementations)
├── benchmark_runner.py        # Run orchestration, variant config, result persistence
├── benchmark_orchestrator.py  # Dashboard-driven parallel execution with progress tracking
├── benchmark_analysis.py      # Scoring, win-rate computation, decision framework
├── benchmark_manifest.py      # Corpus curation, auto-query generation, round-robin selection
├── benchmark_types.py         # Shared data types (SegmentSpec, EmbeddingVector, etc.)
├── audio.py                   # FFmpeg-based decode, segment, resample, energy-window selection
├── vectorstore.py             # Persistent numpy-backed vector store with cosine search
├── vector.py                  # Cosine similarity
└── cli.py                     # CLI: benchmark-init, benchmark-run, benchmark-analyze
```

**Key patterns we can reuse:**

| Pattern | samplebase impl | fourfour reuse |
|---|---|---|
| **Backend abstraction** | `BenchmarkBackend` ABC with `embed_audio_segments()` / `embed_text_query()` / `score()` | New `AnalysisBackend` ABC with `analyze_bpm()` / `analyze_key()` / `analyze_beats()` / `analyze_waveform()` |
| **Variant registry** | `DEFAULT_VARIANTS` dict mapping IDs to backend+config | New variants dict: `stratum_dsp_default`, `essentia_tempocnn`, `madmom_dbn`, `openkeyscan`, etc. |
| **Manifest system** | JSON manifest with `entries[]` (id, path, category, tags, metadata) + `queries[]` | Same shape, but entries become tracks and queries become comparison scenarios |
| **Result persistence** | Per-run dirs with `config.json`, `queries.json`, `results.json`, `scores.json`, `analysis.json` | Exact same structure — swap "embedding scores" for "BPM/key/beat diffs" |
| **Caching layer** | SHA1-keyed JSON embedding cache per backend × segment | Same pattern for caching analysis results per backend × track |
| **Chunking policies** | `full` / `top8s` / `chunked_max` | Not needed for fourfour (we analyze full tracks), but the `SegmentSpec` pattern could model multi-section tracks |
| **VectorStore** | Persistent numpy + JSON meta, cosine search | Reused directly for Phase 5 (CLAP embeddings for similarity search) |
| **Dashboard orchestrator** | Thread-per-backend, progress polling, debug logging | Reused for long-running batch analysis with multiple backends |
| **Audio I/O** | FFmpeg subprocess for decode/segment/resample | fourfour uses symphonia (pure Rust) but could fall back to FFmpeg for Python-side analysis |

### What does NOT transfer
- samplebase benchmarks **retrieval quality** (ranked lists, top-5, MRR) — fourfour benchmarks **signal accuracy** (numeric diff, match/no-match)
- samplebase queries are "find similar" — fourfour queries are "compare to ground truth"
- samplebase scoring is manual human judgment — fourfour scoring is automated diff against Rekordbox export

---

## Phase Overview

```
Phase 0 ── Benchmarking Rig (forked from samplebase)  ← ground truth + harness
Phase 1 ── BPM & Key Accuracy                         ← core DJ features
Phase 2 ── Color Waveforms                             ← biggest visual gap
Phase 3 ── PDB Multi-Page Scaling                      ← unblock real collections
Phase 4 ── Phrase / Structure                          ← next-gen feature
Phase 5 ── Audio Embeddings & Search (samplebase reuse) ← novel differentiator
Phase 6 ── Stems (Optional)                            ← nice-to-have, heavy lift
```

Each phase is self-contained and produces a measurable outcome. Phases 0-3 are **blocking** (must be right before production). Phases 4-6 are **incremental** (add value independently).

---

## Phase 0 — Benchmarking Rig

**Goal:** Build a repeatable test harness to compare analysis outputs against Rekordbox ground truth, forking the samplebase benchmark framework.

### Why First
Every subsequent phase needs a scoring mechanism. Without ground truth, you can't tell if a change improved or regressed anything.

### Strategy: Use external benchmark results → validate stratum-dsp

The external benchmark (samplebase) gave us library recommendations and accuracy data. The remaining work is validating **our** analyzer (stratum-dsp) against Rekordbox ground truth. We don't need to build a full harness from scratch — a minimal comparison script suffices:

```
What's done (samplebase):
  ├── BPM library comparison (DeepRhythm wins, ~97% Acc2)
  ├── Key library comparison (librosa + KS ~70%, OpenKeyScan ~85-90%)
  ├── Energy validation (librosa feature fusion, 19 genres)
  ├── Embedding comparison (MS CLAP vs LAION-CLAP vs TTMR++ vs CLaMP 3)
  └── Python analysis pipeline design with code samples

What's still needed (fourfour):
  ├── Validate stratum-dsp BPM/key/beat accuracy vs Rekordbox export
  ├── Ground truth extraction from master.db + ANLZ (reader/masterdb.rs exists)
  └── If stratum-dsp falls short, integrate winning Python backends
```

### Steps

- [ ] **0.1** Export a reference library from Rekordbox to USB — this becomes the gold standard
- [ ] **0.2** Write a **Rekordbox PDB parser** (`benchmark/pdb_parser.py`) that extracts from the exported USB:
  - BPM, key, duration per track (from `export.pdb`)
  - Track file paths (to match against our corpus)
- [ ] **0.3** Write a **Rekordbox ANLZ parser** (`benchmark/anlz_parser.py`) that extracts:
  - Beat grid positions (from `ANLZ0000.DAT`)
  - Waveform data (PWAV/PWV3/PWV4/PWV5 tags)
  - Phrase markers (if present)
- [ ] **0.4** Fork samplebase's benchmark types and create `AnalysisBackend` interface:
  ```python
  class AnalysisBackend(ABC):
      def analyze_track(self, path: Path) -> AnalysisResult: ...
      def metadata(self) -> dict: ...
  
  @dataclass
  class AnalysisResult:
      bpm: float
      key: str  # "1A", "5B", etc.
      beats: list[float]  # timestamps in seconds
      waveform_preview: bytes  # 400-byte PWAV
      waveform_color: bytes | None  # PWV4/PWV5 data
  ```
- [ ] **0.5** Implement `StratumDspBackend` — spawns the Rust CLI via subprocess, parses JSON output:
  ```bash
  cargo run -p pioneer-usb-writer -- analyze /path/to/track.mp3 --json
  ```
- [ ] **0.6** Implement `GroundTruthQuery` — loads Rekordbox export, matches tracks by filename, produces comparison:
  ```python
  @dataclass
  class TrackComparison:
      track_id: str
      bpm_ours: float; bpm_rb: float; bpm_delta: float
      key_ours: str; key_rb: str; key_match: bool
      beats_count_ours: int; beats_count_rb: int
      beat_offset_mean_ms: float; beat_offset_max_ms: float
      waveform_mse: float | None
  ```
- [ ] **0.7** Curate a **test corpus** of ~30 tracks spanning genres:
  - 10× house/techno (steady 4/4, 120-135 BPM)
  - 5× drum & bass (170-175 BPM, tests half-time detection)
  - 5× hip-hop (80-100 BPM, variable tempo)
  - 5× broken beat / halftime (tests odd meters)
  - 5× pop/rock (live drums, tempo drift)
- [ ] **0.8** Build the manifest using samplebase's pattern:
  ```
  benchmark/
  ├── manifests/
  │   └── corpus-v1.manifest.json    # track list with genre, expected ranges
  │   └── corpus-v1.groundtruth.json # Rekordbox reference data
  ├── results/{run_id}/
  │   ├── config.json                 # backends, timings
  │   ├── comparisons.json            # per-track diffs
  │   └── analysis.json               # aggregated accuracy metrics
  └── cache/                          # per-backend analysis cache (reuse samplebase pattern)
  ```
- [ ] **0.9** Run `stratum-dsp` against the full corpus and record baseline scores

### Reused from samplebase (benchmarking phase, completed)
- Library accuracy data and recommendations → captured in `analysis-pipeline-handoff.md`
- Python analysis pipeline design (BPM, key, energy, waveform code samples)
- Embedding backend implementations for Phase 5

### Remaining work
- Ground truth extraction from Rekordbox master.db (reader exists) + ANLZ (reader needed)
- Minimal comparison script: stratum-dsp output vs ground truth
- Test corpus curation (~30 tracks across genres)

### Deliverable
A ground truth JSON file extracted from Rekordbox, plus a comparison table showing stratum-dsp accuracy. If stratum-dsp falls short, we know which Python backends to integrate (from the handoff doc).

### Time Estimate
1-2 days (ground truth extraction + comparison, much simpler now that library research is done externally)

---

## Phase 1 — BPM & Key Accuracy

**Goal:** Match or exceed Rekordbox accuracy on BPM detection and musical key.

### Current State
Key detection has a current winner: `essentia_key_bgate`.

The Beatport EDM Key benchmark shows `essentia_key_bgate` at 54.0% exact and 68.9% exact-or-adjacent on the 598-track clean single-key subset. The user's Rekordbox baseline on the same Beatport source was 47% exact and 55% exact-or-adjacent on the broader 698-track run.

See [`key-detection-benchmark-findings.md`](./key-detection-benchmark-findings.md).

`stratum-dsp` still needs validation for BPM, beat positions, and possible pure-Rust deployment.

### Step 1A — Validate stratum-dsp

- [ ] **1A.1** Run Phase 0 benchmark against full test corpus using `StratumDspBackend`
- [ ] **1A.2** Review `analysis.json` output — categorize errors:
  - **Octave errors** (64 vs 128 BPM) — most critical
  - **Small drift** (128.0 vs 128.5 BPM)
  - **Key mismatches** (1A vs 1B, relative major/minor confusion)
  - **Complete misses** (beyond salvageable)
- [ ] **1A.3** If stratum-dsp scores ≥ 90% BPM accuracy and matches `essentia_key_bgate` key quality → pure Rust remains viable
- [ ] **1A.4** If not, use the Python sidecar for key detection

### Step 1B — Python Analysis Backends (if needed)

For key detection, the Python sidecar path is already justified by the Beatport benchmark.

For public CLI contract validation, use the tracked batch runner:

```bash
analysis/.venv/bin/python benchmark/scripts/cli_batch_analyze.py /path/to/audio --tmux
```

The script writes generated outputs under `benchmark/results/` and logs under `benchmark/logs/`.

```
┌──────────────┐     JSON/stdio      ┌──────────────────────┐
│  Rust app    │ ◄──────────────────► │  Python analysis     │
│  (Tauri)     │                      │  (benchmark sidecar) │
└──────────────┘                      │                      │
                                      │  StratumDspBackend   │ ← subprocess → cargo run
                                      │  EssentiaKeyBackend  │ ← KeyExtractor bgate
                                      │  MadmomBackend       │ ← DBNBeatTracker
                                      │  OpenKeyScanBackend  │ ← CNN key detection
                                      └──────────────────────┘
```

- [x] **1B.1** Create a `fourfour/analysis/` Python package (separate from the Rust crate)
- [x] **1B.2** Implement key-detection backends:
  - **DeepRhythm** — BPM detection (97% Acc2, ~0.2s/track)
  - **librosa chroma_cqt + KS** — historical key baseline
  - **Essentia KeyExtractor bgate** — current key winner
  - **Essentia KeyExtractor profile variants** — benchmarked historically, not exposed as public variants
- [x] **1B.3** Register as variants:
  ```python
  ANALYSIS_VARIANTS = {
      "stratum_dsp_default": {"backend": "stratum_dsp", "label": "stratum-dsp (Rust baseline)"},
      "deeprhythm":          {"backend": "deeprhythm",  "label": "DeepRhythm (97% Acc2)"},
      "essentia_key_bgate":  {"backend": "essentia_key", "label": "Essentia KeyExtractor bgate"},
  }
  ```
- [x] **1B.4** Run all key backends against the Beatport corpus
- [x] **1B.5** Review per-backend key accuracy

### Step 1C — ONNX Runtime (future production path)

If a Python backend wins but we don't want the Python dependency in production:

- [ ] Convert winning model to ONNX format
- [ ] Use `ort` crate (ONNX Runtime Rust bindings) to run inference natively
- [ ] This is a **follow-up optimization**, not a Phase 1 requirement

### Step 1D — Octave Error Guard

Regardless of which engine wins:

- [ ] **1D.1** Implement BPM range validation:
  - If BPM < 70 or BPM > 200, check for double/half
  - Cross-validate beat grid density against BPM (grid should have beats every 60/BPM seconds)
- [ ] **1D.2** Implement key validation:
  - If key is between 1A-12A, verify by checking the relative major/minor
  - Use genre-aware priors (house ≈ 120-135 BPM, DnB ≈ 170-180)

### Decision Gate
Run benchmark. If BPM hits the target and key detection at least matches Rekordbox → **Phase 1 analysis quality is acceptable**.

### Time Estimate
- Step 1A: 1-2 days (ground truth extraction + comparison)
- Step 1B: 2-4 days (if needed — pipeline code samples exist in handoff doc)
- Step 1D: 1 day

---

## Phase 2 — Color Waveforms

**Goal:** Generate PWV3 (blue/mono), PWV4 (RGB), and PWV5 (full color) waveform data that matches CDJ display quality.

### Why This Matters
The waveform is the primary visual feedback on CDJ. Currently we write hardcoded green. This is the most obvious visual difference vs a Rekordbox export.

### Understanding the Formats

From the Pioneer docs and reverse engineering:

| Tag | Size | Description |
|---|---|---|
| **PWAV** | 400 bytes | Monochrome preview (✅ we have this) |
| **PWV3** | ~16KB | Blue 3-band waveform (low/mid/high frequency) |
| **PWV4** | ~16KB | RGB waveform (frequency → color mapping) |
| **PWV5** | ~16KB | Full-color waveform with more color resolution |

### Step 2A — Capture Rekordbox Ground Truth

- [ ] **2A.1** Export a few tracks from Rekordbox with full analysis
- [ ] **2A.2** Parse the ANLZ PWV3/PWV4/PWV5 tags from Rekordbox output
- [ ] **2A.3** Reverse-engineer the color encoding:
  - What frequency ranges map to what colors?
  - How is amplitude encoded per band?
  - What's the exact byte layout?

### Step 2B — Spectral Analysis Pipeline

- [ ] **2B.1** Implement FFT-based frequency band splitting in Rust:
  - Low band: 0-300 Hz (bass/kick)
  - Mid band: 300-2000 Hz (melody/vocals)
  - High band: 2000-20000 Hz (hi-hats/cymbals)
- [ ] **2B.2** For each waveform column (time slice):
  - Compute RMS per frequency band
  - Map to color channels per format:
    - PWV3: blue intensity per band
    - PWV4: RGB from frequency → hue mapping
    - PWV5: extended color space
- [ ] **2B.3** Candidate Rust crates:
  - `rustfft` — pure Rust FFT, mature
  - `realfft` — real-valued FFT wrapper
  - Or continue with `symphonia` decoded audio + custom FFT

### Step 2C — Validate on Hardware

- [ ] **2C.1** Write a test USB with our generated waveforms
- [ ] **2C.2** Compare visually on CDJ-3000 screen against Rekordbox export
- [ ] **2C.3** Iterate on color mapping until perceptually similar

### Decision Gate
CDJ shows recognizable color waveform (not flat green) → **Phase 2 complete**.

### Time Estimate
5-8 days (depends on how much reverse engineering PWV5 needs)

---

## Phase 3 — PDB Multi-Page Scaling

**Goal:** Support real collections (100+ tracks, playlists) by implementing multi-page PDB writes.

### Why This Matters
Current PDB writer pre-allocates one 4096-byte page per table. At ~344 bytes per track row, that's ~10 tracks max before overflow. This is the hard blocker for anything beyond a tech demo.

### Steps

- [ ] **3.1** Study the Rekordbox-exported PDB file structure for multi-page tables:
  - How are page chains linked?
  - How does `next_page` / `prev_page` work in the page header?
  - What's the page 0 sequence number strategy for multi-page data?
- [ ] **3.2** Refactor `pdb.rs` to support dynamic page allocation:
  - Track allocated pages in a `PageAllocator`
  - When a page fills, allocate a new one and link it
  - Update page 0 (tables-of-tables) with correct row counts and page spans
- [ ] **3.3** Test with progressively larger collections:
  - 10 tracks → 50 tracks → 200 tracks → 1000 tracks
- [ ] **3.4** Verify on CDJ-3000 at each step — the CDJ is the only validator that matters
- [ ] **3.5** Handle the columns table (0x10) special case — it uses a different header format

### Decision Gate
Write 200+ tracks to USB, CDJ reads the full collection without errors → **Phase 3 complete**.

### Time Estimate
5-7 days

---

## Phase 4 — Phrase / Structure Analysis

**Goal:** Automatically detect intro, breakdown, drop, verse, outro sections in tracks.

### Why This Matters
Rekordbox's phrase analysis is a key selling point. DJs use it for navigation. No open-source tool does this well for electronic music.

### Step 4A — Baseline with Existing Tools

- [ ] **4A.1** Run MSAF on test corpus, evaluate boundary detection quality
- [ ] **4A.2** Try Essentia's self-similarity + novelty detection
- [ ] **4A.3** Map MSAF's similarity labels (A, B, A', C) to DJ labels (intro, breakdown, drop, outro)

### Step 4B — Custom DJ Music Pipeline

Electronic music has strong structural conventions we can exploit:

- [ ] **4B.1** Heuristic rules for DJ section detection:
  - **Intro**: first 16-32 bars, usually sparse, builds energy
  - **Breakdown**: energy drops, often removes kick drum
  - **Build/Rise**: energy increases after breakdown
  - **Drop**: peak energy return, loudest section
  - **Outro**: final 16-32 bars, energy fades
- [ ] **4B.2** Feature engineering per section:
  - RMS energy envelope (detect breakdowns/drops)
  - Spectral centroid (detect when hi-hats enter/exit)
  - Kick drum presence detection (low band energy periodicity)
- [ ] **4B.3** Build a beat-synchronous feature pipeline:
  - One feature vector per beat (or per bar, or per 4-bar phrase)
  - Feed into a simple threshold-based classifier first
- [ ] **4B.4** If threshold approach is insufficient, train a lightweight classifier:
  - Label 50-100 tracks manually with phrase annotations
  - Train a small decision tree or SVM on beat-synchronous features
  - This doesn't need deep learning — the feature space is small

### Step 4C — Write to Pioneer Format

- [ ] **4C.1** Determine how Rekordbox stores phrase data in ANLZ/PDB
- [ ] **4C.2** Implement phrase tag writing in `anlz.rs`
- [ ] **4C.3** Validate on CDJ

### Decision Gate
Phrase markers visible on CDJ waveform display, sections correspond to actual song structure → **Phase 4 complete**.

### Time Estimate
10-15 days (most research-heavy phase)

---

## Phase 5 — Audio Embeddings & Similarity Search

**Goal:** Enable "find tracks that sound like this one" and text-based search ("dark minimal techno").

### Big head start from samplebase

The external benchmark (samplebase) already implemented and compared CLAP backends. The code and results are documented in `analysis-pipeline-handoff.md`. Key findings:

- **MS CLAP** — best for text+audio (MRR 0.667, Hit@5 0.600, 658 MB)
- **LAION-CLAP** — best for audio→audio (Hit@5 0.791, 1778 MB)
- **TTMR++** — runner-up (128-dim, 1854 MB)
- **CLaMP 3** — collapsed embeddings, do not use

The transfer plan:

### Steps

- [ ] **5.1** Copy `samplebase/mvp/src/samplebase_mvp/vectorstore.py` → fourfour (or import as dependency)
- [ ] **5.2** Implement CLAP backends based on the patterns from the handoff doc (MS CLAP primary, LAION-CLAP for audio→audio)
- [ ] **5.3** Adapt the chunking policy for full DJ tracks:
  - samplebase's `top8s` (energy-based 8-second window) maps to "pick the most energetic section"
  - samplebase's `chunked_max` (overlapping 8s chunks) maps to "embed at 10%, 45%, 80% of track" (CLAP standard for full tracks)
  - `full` works as-is for shorter tracks
- [ ] **5.4** Run CLAP on the fourfour test corpus, generate embeddings
- [ ] **5.5** Evaluate quality:
  - Pick a track, find top-5 similar → are they actually genre/vibe compatible?
  - Text search "deep house" → does it return deep house tracks?
  - Reuse samplebase's scoring workflow (manual top-5 relevance judgment)
- [ ] **5.6** Storage: reuse `VectorStore` with SQLite metadata (path, artist, title, genre, BPM, key)
- [ ] **5.7** Try Microsoft CLAP `clapcap` for auto-generating track descriptions
- [ ] **5.8** Build a simple UI for similarity browsing (can stay in test UI)

### Architecture

```
┌─────────────────────────────────────────────────────────┐
│  fourfour analysis (Python)                             │
│                                                         │
│  Full DJ track ──▶ CLAP embed ──▶ 512-dim vector        │
│       │                                  │              │
│       │                    stored in VectorStore         │
│       │                    (from samplebase)             │
│       │                                  │              │
│       └──▶ text query "dark techno" ──▶ cosine search   │
│       └──▶ audio query reference.wav ──▶ cosine search  │
└─────────────────────────────────────────────────────────┘
```

### Decision Gate
"Find similar" returns perceptually relevant results for ≥80% of queries → **Phase 5 complete**.

### Time Estimate
3-4 days (down from 5-8, since backends + vectorstore + scoring come from samplebase)

---

## Phase 6 — Stems / Source Separation (Optional)

**Goal:** Allow DJs to isolate instruments (drums, bass, vocals, melody) from any track.

### Why Last
This is the heaviest lift (computationally and dependency-wise). It's a differentiator but not essential for MVP.

### Steps

- [ ] **6.1** Evaluate Demucs v4 (Python/PyTorch):
  - Quality: state-of-the-art SDR
  - Speed: ~30s per 5-min track on GPU, much slower on CPU
  - License: MIT ✅
- [ ] **6.2** Evaluate demucs-rs (Rust/Burn):
  - Native integration, no Python
  - Less mature, same model weights
  - Metal support on macOS ✅
- [ ] **6.3** Decide on integration model:
  - **Option A:** On-demand stem export (user clicks "separate", waits)
  - **Option B:** Background batch processing during import
  - **Option C:** Pre-computed stems stored alongside tracks
- [ ] **6.4** Figure out how to write stems to Pioneer USB (if at all supported)
- [ ] **6.5** If not Pioneer-compatible, ship as app-only feature

### Decision Gate
Stems play back cleanly with <1% audible artifacts → **Phase 6 complete**.

### Time Estimate
8-12 days (if pursued)

---

## Architecture Decision Matrix

After Phases 0-1, we'll know enough to decide on the long-term architecture:

### Scenario A: stratum-dsp Is Good Enough

```
pioneer-usb-writer (Rust)
├── symphonia     → audio decode
├── lofty         → metadata
├── stratum-dsp   → BPM, key, beats
├── rustfft       → color waveforms
├── image         → artwork resize
└── custom code   → PDB, ANLZ, filesystem

No Python. No sidecar. Ship as single binary.

Phase 5 (embeddings) still uses Python CLAP backends from samplebase,
but runs as a separate optional process for similarity search only.
```
**Pros:** Simplest deployment. Fastest analysis. No dependency hell.
**Cons:** Locked into stratum-dsp quality. No ML model upgrades without ONNX work.

### Scenario B: Python Analysis Sidecar Required

```
pioneer-usb-writer (Rust)
├── symphonia, lofty, image, custom PDB/ANLZ
└── spawns → fourfour-analysis (Python)
              ├── DeepRhythm → BPM (97% Acc2)
              ├── Essentia KeyExtractor bgate → key
              ├── librosa + KS → historical key baseline
              ├── OpenKeyScan/CNN → future candidate only after real benchmark
              └── MS CLAP → embeddings (from handoff doc)

Analysis pipeline design from external benchmarking.
```
**Pros:** Best analysis quality. Easy to swap models. Pipeline code samples exist in handoff doc.
**Cons:** Python dependency management. ~500MB+ bundled size. Slower startup.

### Scenario C: ONNX Runtime (Future Production)

```
pioneer-usb-writer (Rust)
├── ort (ONNX Runtime) → runs converted ML models natively
├── rustfft → waveforms
├── symphonia, lofty → audio/metadata
└── custom PDB/ANLZ

No Python at runtime. Models downloaded or bundled.
```
**Pros:** Best of both worlds — ML quality without Python.
**Cons:** Model conversion is non-trivial. ONNX Runtime adds ~100MB.

### Cross-project reference summary

| Component | Source | Status |
|---|---|---|
| Library accuracy data | samplebase external benchmark | ✅ In `analysis-pipeline-handoff.md` |
| Python analysis pipeline design | samplebase external benchmark | ✅ Code samples in handoff doc |
| CLAP backend implementations | samplebase `benchmark_backends.py` | Available to copy when Phase 5 starts |
| VectorStore | samplebase `vectorstore.py` | Available to copy when Phase 5 starts |
| Result directory structure | samplebase pattern | Documented in `benchmark-implementation-plan.md` |

---

## Quick Reference: Experiment Commands

```bash
# Phase 0: Build corpus + extract ground truth from Rekordbox USB
python -m fourfour.analysis.groundtruth_export /Volumes/REKORDBOX_USB \
  --output benchmark/manifests/corpus-v1.groundtruth.json

# Phase 0: Build manifest from track corpus
python -m fourfour.analysis.corpus_init ~/Music/benchmark-corpus \
  --name corpus-v1

# Phase 0/1: Run all analysis backends against corpus
python -m fourfour.analysis.benchmark_run \
  --manifest benchmark/manifests/corpus-v1.manifest.json \
  --groundtruth benchmark/manifests/corpus-v1.groundtruth.json \
  --variants stratum_dsp_default essentia_tempocnn madmom_dbn

# Phase 0/1: Re-analyze after manual scoring (samplebase pattern)
python -m fourfour.analysis.benchmark_analyze run-20260417T120000Z

# Phase 1: Compare single track across engines
cargo run -p pioneer-usb-writer -- analyze /path/to/track.mp3 --json
python -m fourfour.analysis.analyze_track /path/to/track.mp3
python -m fourfour.analysis.analyze_track /path/to/track.mp3 --backend madmom

# Phase 2: Generate + compare waveforms
cargo run -p pioneer-usb-writer -- waveform /path/to/track.mp3 --format pwv5 --output /tmp/waveform.bin

# Phase 3: Stress test PDB
cargo run -p pioneer-usb-writer -- write /path/to/big-library -o /Volumes/USB --max-tracks 1000

# Phase 5: Embeddings (reuses samplebase CLAP backends)
python -m fourfour.analysis.embed_corpus benchmark/manifests/corpus-v1.manifest.json \
  --backend laion_clap_full
python -m fourfour.analysis.search "dark minimal techno"
python -m fourfour.analysis.similar /path/to/reference-track.mp3
```

---

## Dependency Risk Map

| Dependency | License | Risk | Mitigation |
|---|---|---|---|
| `stratum-dsp` | Check crate | Low quality → need sidecar | Phase 1 validates |
| `essentia` | Check exact package/license | License and native wheel risk | Keep sidecar boundary available |
| `madmom` | BSD-like academic | Maintenance risk | Pin versions |
| `openkeyscan` | Check repo | New, unproven | Benchmark before adopting |
| `MSAF` | MIT | Unmaintained | Fork + fix deps |
| `LAION CLAP` | Apache 2.0 | Model size | Download on demand |
| `Demucs` | MIT | Archived upstream | Use maintained fork |
| `demucs-rs` | Check repo | Immature | Fallback to Python |
| `rustfft` | MIT | Stable | Low risk |
| `ort` (ONNX) | MIT | Model compat | Test conversion early |

---

## Success Metrics

| Metric | Target | How to Measure |
|---|---|---|
| BPM accuracy | ≥ 95% within ±0.5 BPM of Rekordbox | Phase 0 benchmark |
| Key accuracy | At least match Rekordbox on external labels | Beatport benchmark |
| Octave errors | < 1% of tracks | Phase 1 benchmark |
| Color waveform | Perceptually matches Rekordbox on CDJ | Visual inspection |
| PDB capacity | ≥ 1000 tracks on one USB | Phase 3 stress test |
| Phrase labeling | ≥ 70% of sections correctly identified | Manual evaluation on 30 tracks |
| Similarity search | ≥ 80% of top-5 results are genre/vibe matched | Manual evaluation |
| Total analysis time | < 5s per track (excl. stems) on M-series Mac | Stopwatch |
