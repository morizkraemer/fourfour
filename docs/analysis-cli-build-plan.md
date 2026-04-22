# fourfour — Analysis CLI Build Plan

> One doc. Phased. Every step is verifiable. No step depends on a later step.
>
> **Status:** Pre-implementation. No `analysis/` directory exists yet.
> **Estimate:** ~31 hours across ~4 focused days.

---

## What We're Building

A Python CLI (`fourfour-analysis`) that analyzes audio files and benchmarks the results against ground truth. Three analysis backends compete head-to-head:

| Backend | BPM | Key | Energy | Waveform | Cues | Heavy deps |
|---------|-----|-----|--------|----------|------|------------|
| **lexicon_port** | onset+autocorrelation | custom chroma + KS | RMS+tempo+transient | 128-pt FFT, 3-band | energy phrase | numpy, scipy |
| **python_deeprhythm** | DeepRhythm (torch) | librosa chroma + KS | librosa features | numpy FFT, 3-band | — | torch, librosa |
| **stratum_dsp** | Rust subprocess | Rust subprocess | — | 400-byte preview | — | Rust binary |

The benchmark tells us which to ship (or which hybrid combination).

### Key decisions this benchmark answers

1. Can Lexicon's onset+autocorrelation match DeepRhythm's 97% Acc2?
2. Is Lexicon's custom chroma better/worse than librosa's ~70%?
3. How much faster is the downsampled Lexicon approach (200Hz/4.4kHz)?
4. Does any hybrid combination (e.g. DeepRhythm BPM + Lexicon everything-else) win?

---

## Final Directory Structure

```
fourfour/
├── analysis/                              ← Python package
│   ├── pyproject.toml
│   ├── src/fourfour_analysis/
│   │   ├── __init__.py
│   │   ├── __main__.py                    ← python -m fourfour_analysis
│   │   ├── cli.py                         ← argparse: fourfour-analyze, fourfour-benchmark
│   │   ├── config.py                      ← Settings, path resolution
│   │   ├── types.py                       ← TrackEntry, AnalysisResult, GroundTruth, etc.
│   │   ├── cache.py                       ← SHA1-keyed JSON cache
│   │   ├── audio_io.py                    ← load/resample/filter (shared by all backends)
│   │   ├── backends/
│   │   │   ├── __init__.py
│   │   │   ├── base.py                    ← AnalysisBackend ABC
│   │   │   ├── registry.py               ← ANALYSIS_VARIANTS dict + load_backend()
│   │   │   ├── python_stack.py            ← DeepRhythm + librosa
│   │   │   ├── lexicon_bpm.py             ← Onset + autocorrelation BPM
│   │   │   ├── lexicon_key.py             ← Custom chroma + KS key
│   │   │   ├── lexicon_energy.py          ← RMS + tempo + transient energy
│   │   │   ├── lexicon_waveform.py        ← 128-pt FFT, 3-band waveform
│   │   │   ├── lexicon_cues.py            ← Energy phrase segmentation
│   │   │   ├── lexicon_port.py            ← Wires lexicon_* modules into backend
│   │   │   └── stratum_dsp.py             ← Rust subprocess wrapper
│   │   ├── manifest.py                    ← build corpus from tagged audio
│   │   ├── groundtruth.py                 ← extract BPM/key from ID3/Vorbis tags
│   │   ├── compare.py                     ← diff analysis vs ground truth
│   │   ├── runner.py                      ← benchmark run orchestration
│   │   └── scoring.py                     ← aggregate metrics + recommendation
│   └── tests/
│       ├── test_cache.py
│       ├── test_lexicon_bpm.py
│       ├── test_lexicon_key.py
│       ├── test_lexicon_energy.py
│       ├── test_lexicon_waveform.py
│       ├── test_compare.py
│       └── fixtures/                      ← generated audio for unit tests
│           ├── sine_440_10s.wav
│           └── click_track_128bpm.wav
├── benchmark/                             ← data (gitignored)
│   ├── manifests/                         ← *.corpus.json
│   ├── results/{run_id}/                  ← per-run outputs
│   └── cache/                             ← content-addressed analysis cache
└── docs/
    └── analysis-cli-build-plan.md         ← this file
```

### Cross-reference docs (read-only reference during build)

| Doc | What it contains | When to read |
|-----|-----------------|--------------|
| `lexicon-wiki.md` | Reverse-engineered Lexicon algorithm details (BPM, key, energy, waveform, cues) | Before Steps 6–10 |
| `lexicon-reverse-engineering.md` | Raw decompiled source, constants, profiles | During Steps 6–10 |
| `analysis-pipeline-handoff.md` | Python stack code samples, library choices, accuracy benchmarks | Before Steps 13–14 |
| `tech-stack-reference.md` | Library survey (Essentia, madmom, OpenKeyScan, etc.) | If exploring alternatives |

---

## System Flow

### `fourfour-analyze` — single file

```
audio file
    │
    ▼
audio_io.load_audio()              ← soundfile (WAV/FLAC) or ffmpeg pipe (MP3/AAC)
    │
    ├─ preprocess_tempo()  ─► lexicon_bpm.analyze_tempo()    ─► bpm, beats
    ├─ preprocess_key()    ─► lexicon_key.detect_key()       ─► key (Camelot)
    ├─ preprocess_tempo()  ─► lexicon_energy.compute()       ─► energy (1–10)
    ├─ preprocess_waveform() ► lexicon_waveform.generate()   ─► peaks[], colors[]
    └─ beats + audio       ─► lexicon_cues.detect()          ─► cue_points[]
                                    │
                                    ▼
                          AnalysisResult → stdout (JSON or table)
```

### `fourfour-benchmark init` — build corpus from tagged library

```
~/Music/corpus/
    ├── track01.mp3   [TBPM=128, TKEY=Abm]     ← ID3/Vorbis tags = ground truth
    ├── track02.mp3   [TBPM=140, TKEY=5A]
    └── track03.mp3   [no BPM tag]              ← analyzed but not scored
         │
         ▼
manifest.py: scan + mutagen tag extraction
    │
    ▼
benchmark/manifests/accuracy-v1.corpus.json
    {
      entries: [
        { id, path, fingerprint, artist, title, genre,
          ground_truth: { bpm: 128.0, bpm_source: "tag", key: "1A", key_source: "tag",
                          energy: null, waveform_quality: null, cue_points: null } },
        ...
      ]
    }
```

Ground truth comes **only from file tags**. Fields tags can't provide (waveform_quality, cue_points) are `null` — manual assessment only.

### `fourfour-benchmark run` — analyze + compare + score

```
corpus.json
    │
    ▼
runner.py
    ├─ for each backend:
    │    ├─ for each track:
    │    │    analyze_track_cached()  ← cache hit? skip; miss? analyze + cache
    │    └─ write results/{run_id}/raw/{backend_id}.json
    ├─ compare.py: BPM diff + key match per (track × backend)
    │    └─ write comparisons.json
    └─ scoring.py: aggregate → scoring.json + recommendation
```

---

## Phase 0 — Scaffold

*Produces: importable package, CLI stub, types, audio I/O, cache.*

---

### Step 1: Package skeleton

**Files:** `analysis/pyproject.toml`, `src/fourfour_analysis/{__init__,__main__,cli}.py`

```bash
cd analysis
python3 -m venv .venv && source .venv/bin/activate
pip install -e ".[dev]"
fourfour-analyze --help    # exits 0
fourfour-benchmark --help  # exits 0
```

**pyproject.toml key sections:**
```toml
[project]
name = "fourfour-analysis"
version = "0.1.0"
requires-python = ">=3.11"
dependencies = ["numpy>=1.26", "scipy>=1.12", "soundfile>=0.12"]

[project.optional-dependencies]
ml = ["torch>=2.0", "librosa>=0.10", "deeprhythm", "mutagen>=1.47"]
dev = ["pytest>=8.0", "pytest-timeout>=2.2"]

[project.scripts]
fourfour-analyze = "fourfour_analysis.cli:analyze_main"
fourfour-benchmark = "fourfour_analysis.cli:benchmark_main"
```

---

### Step 2: Type definitions

**File:** `src/fourfour_analysis/types.py`

Frozen dataclasses — no logic, just shapes:

- `TrackEntry` — id, path, label, genre, duration, content_fingerprint, artist, title
- `AnalysisResult` — bpm, key, energy, beats, waveform_peaks, waveform_color, cue_points, elapsed_seconds, backend_metadata
- `AnalysisRecord` — track_id, backend_id, status (ok/failed/timeout), result, error
- `GroundTruth` — track_id, bpm, key, energy, beats, cue_points, bpm_source, key_source
- `TrackComparison` — per-track diff: bpm_delta, key_match, energy_delta, beat_f_measure
- `BackendMetadata` — id, label, version, config_hash, heavy_deps, network_required

**Verify:** `python -c "from fourfour_analysis.types import *"`

---

### Step 3: Config

**File:** `src/fourfour_analysis/config.py`

```python
@dataclass(frozen=True)
class Settings:
    root_dir: Path  # fourfour project root (walks up from cwd to find Cargo.toml)
    # Properties: benchmark_dir, manifests_dir, results_dir, cache_dir
```

**Verify:** `fourfour-analyze config-dirs` prints resolved paths.

---

### Step 4: Audio I/O

**File:** `src/fourfour_analysis/audio_io.py`

Shared audio loading + preprocessing. Used by all backends.

| Function | Purpose |
|----------|---------|
| `load_audio(path, sr=None)` | mono f32, soundfile for WAV/FLAC, ffmpeg pipe for MP3/AAC |
| `resample(audio, sr_from, sr_to)` | `scipy.signal.resample_poly` |
| `lowpass_cascade(audio, sr, freqs)` | N-stage Butterworth via `scipy.signal.butter` + `sosfilt` |
| `preprocess_tempo(audio, sr)` | 7-stage cascade: [800,400,400,200,200,200,200] Hz → <200Hz |
| `preprocess_key(audio, sr)` | FIR lowpass 110 taps @ 1999Hz + 10:1 decimate → ~4.4kHz |
| `preprocess_waveform(audio, sr)` | Resample to 12kHz mono |

**Test fixtures:** Generate `sine_440_10s.wav` and `click_track_128bpm.wav` in tests/fixtures.

**Verify:** All 6 functions have passing tests. MP3 load skips if ffmpeg not on PATH.

---

### Step 5: Cache

**File:** `src/fourfour_analysis/cache.py`

```python
def cache_key(content_fingerprint, backend_id, config_hash) -> str:
    # SHA1(content_fingerprint + backend_id + config_hash)[:24]

def load_cache(cache_dir, key) -> AnalysisRecord | None
def save_cache(cache_dir, key, record) -> None
```

**Verify:** `pytest tests/test_cache.py` passes.

---

## Phase 1 — Lexicon Algorithm Port

*Produces: 5 standalone modules, each independently testable.*
*Each is a pure function: `numpy array → result`.*
*Reference: `docs/lexicon-wiki.md` for algorithm details.*

**Steps 6–9 are independent — build in any order or in parallel.**

---

### Step 6: BPM detection — `lexicon_bpm.py`

**Reference:** `lexicon-wiki.md` §4, Worker 182 Module 745

**Algorithm pipeline:**

1. **`spectral_flux(audio, sr, tempo_max)`** — sliding window energy comparison
   - `window_size = (30 / tempo_max) * sr`
   - `flux = max(0, (1 - old/new) * new) / (window_size/2)`

2. **`find_peaks(flux, threshold)`** — adaptive threshold, decrement 5% of avg until enough peaks

3. **`interval_histogram(peaks, sr, tempo_min, tempo_max)`** — pair peaks within 10 positions → BPM candidates, score by count + ±0.5 BPM neighbors

4. **`autocorrelation(audio, sr, bpm_candidates)`** — correlate at 4, 8, 16 beat multiples, average

5. **`resolve_octave(bpm, candidates, corr_func)`** — test 1.5×, 0.67×, 2×, 0.5×; genre heuristics (DnB ≈ 174)

6. **`fine_tune(bpm, audio, sr)`** — snap to integer if corr ≥ 95%; search ±0.05 in 0.001 steps

**Test cases:**

| Input | Expected |
|-------|----------|
| Click track at 100 BPM | 100.0 ± 1.0 |
| Click track at 128 BPM | 128.0 ± 1.0 |
| Click track at 140 BPM | 140.0 ± 1.0 |
| Click track at 174 BPM | 174.0 ± 1.0 |
| Pure 440Hz sine | bpm=None or status=failed |
| Half-time 85 BPM | 85 or 170 (either valid) |

**Verify:** `pytest tests/test_lexicon_bpm.py` passes.

---

### Step 7: Key detection — `lexicon_key.py`

**Reference:** `lexicon-wiki.md` §5, Worker 182 functions O(), W(), E(), j(), C()

**Constants (from reverse engineering):**
```python
MAJOR_PROFILE = [7.24, 3.50, 3.58, 2.85, 5.82, 4.56, 2.45, 6.99, 3.39, 4.56, 4.07, 4.46]
MINOR_PROFILE = [7.00, 3.14, 4.36, 5.40, 3.67, 4.09, 3.91, 6.20, 3.63, 2.87, 5.35, 3.83]
OCTAVE_WEIGHTS = [0.400, 0.556, 0.525, 0.608, 0.599, 0.491]
```

**Algorithm pipeline:**

1. Slice audio: 25% offset, 50% duration
2. FIR lowpass: 110 taps, cutoff ≈ 1999 Hz
3. Decimate by ~10:1 → ~4400 Hz effective
4. Frame: Blackman window, 16384 samples, 4096 hop
5. FFT per frame: `np.fft.rfft(frame[:2048])` — only first 2048 of 16384 (matches Lexicon)
6. Triangular kernel binning → 72 pitch classes (6 octaves × 12 semitones), bandwidth = 0.9 semitones
7. Average chroma: RMS-weighted mean across frames
8. Krumhansl-Schmuckler: 24 rotations, profiles × OCTAVE_WEIGHTS, Pearson correlation
9. Convert to Camelot: `MAJOR_CAMELOT = {0: "8B", 1: "3B", ...}`

**Test cases:**

| Input | Expected |
|-------|----------|
| C major chord (C+E+G) | C major family |
| A minor chord (A+C+E) | A minor family |
| 440Hz (A4) | Key contains A |

**Verify:** `pytest tests/test_lexicon_key.py` passes.

---

### Step 8: Energy rating — `lexicon_energy.py`

**Reference:** `lexicon-wiki.md` §8, Worker 182 function A()

**Algorithm:**
```python
def compute_energy(audio, sr, bpm, drop_regions=None) -> int:  # 1-10
    # 1. RMS in drop regions (or 30-70% of track) → 50% weight
    # 2. Tempo factor: (bpm - 120) / 120 → 30% weight
    # 3. Transient density:
    #    - 0.014s windows, RMS per segment
    #    - Strong beat: rise > 0.2 AND rms > 0.3
    #    - density = strong_per_second × 60
    #    - normalize: clamp((density - 550000) / 150000, 0, 1) → 50% weight
    # Penalty: if strong_beats <= 200 → score *= 0.2
    # Output: clamp(round(9 * score) + 1, 1, 10)
```

**Test cases:**

| Input | Expected |
|-------|----------|
| Click track at 140 BPM | energy ≥ 6 |
| Low-amplitude sine | energy ≤ 3 |

**Verify:** `pytest tests/test_lexicon_energy.py` passes.

---

### Step 9: Waveform — `lexicon_waveform.py`

**Reference:** `lexicon-wiki.md` §9, Worker 160

**Constants:**
```python
TARGET_SR = 12000
FFT_SIZE = 128
SEGMENT_WIDTH = 256  # samples per output column
LOW_BAND = (0, 150)      # Hz
MID_BAND = (150, 1500)   # Hz
HIGH_BAND = (1500, 6000) # Hz (Nyquist at 12kHz)
MIX_FACTOR = 0.5  # blend with previous segment
```

**Per segment:**
1. Min/max of 256 samples → waveform shape
2. 128-point FFT → spectrum
3. RMS of bins in each band → raw energy
4. Normalize to strongest band → RGB (0-255)
5. Blend with previous segment (50%)

**Test cases:**

| Input | Expected |
|-------|----------|
| 10s audio at 12kHz | `len(peaks) ≈ 12000*10/256 ≈ 468` |
| All zeros | all peaks = (0,0), all colors = (0,0,0) |
| 80Hz sine | red channel dominant |

**Verify:** `pytest tests/test_lexicon_waveform.py` passes.

---

### Step 10: Cue points — `lexicon_cues.py`

**Reference:** `lexicon-wiki.md` §7, Worker 182 function M()

**Depends on:** Step 6 (needs beat positions from BPM module)

**Algorithm:**
1. Per bar (4 beats at BPM): RMS energy, beat strength (avg RMS ±50ms), ramp type (regression slope)
2. Segment: split at mean energy → high/low sections
3. Filter: min section = 64 beats, round to 4-bar boundaries
4. Assign labels: Start, Drop, Breakdown, SecondDrop, Lastbeat
5. Emergency loop: find 16-beat stable section before last drop

**Test cases:**

| Input | Expected |
|-------|----------|
| Any track | First cue at time ≈ 0 |
| 6-min house track | ≥ 4 cue points |

**Verify:** `pytest tests/test_lexicon_cues.py` passes.

---

## Phase 2 — Backend Integration

*Produces: 3 runnable backends behind a common interface.*

**Steps 12–14 are independent — build in any order.**

---

### Step 11: ABC + Registry

**Files:** `backends/{__init__,base,registry}.py`

**`base.py`** — `AnalysisBackend` ABC:
```python
class AnalysisBackend(ABC):
    @abstractmethod
    def metadata(self) -> BackendMetadata: ...
    @abstractmethod
    def analyze_track(self, track_path: str) -> AnalysisResult: ...

    def analyze_track_cached(self, track: TrackEntry) -> AnalysisRecord:
        key = cache_key(track.content_fingerprint, self.metadata().id, self.metadata().config_hash)
        cached = load_cache(self.cache_dir, key)
        if cached: return cached
        # ... analyze, save, return
```

**`registry.py`** — variant definitions + factory:
```python
ANALYSIS_VARIANTS = {
    "python_deeprhythm": { "backend": "python_stack", "label": "DeepRhythm + librosa KS", ... },
    "lexicon_port":      { "backend": "lexicon_port", "label": "Lexicon algorithms (Python port)" },
    "stratum_dsp":       { "backend": "stratum_dsp",  "label": "stratum-dsp (Rust subprocess)" },
}

def load_backend(variant_id: str, settings: Settings) -> AnalysisBackend: ...
```

**Verify:** `load_backend("lexicon_port", settings)` returns instance.

---

### Step 12: LexiconPortBackend

**File:** `backends/lexicon_port.py`

Wires Steps 6–10 into one `AnalysisBackend`:

```python
def analyze_track(self, track_path: str) -> AnalysisResult:
    audio, sr = load_audio(track_path)
    tempo_audio = preprocess_tempo(audio, sr)
    key_audio = preprocess_key(audio, sr)
    waveform_audio = preprocess_waveform(audio, sr)

    bpm_result = lexicon_bpm.analyze_tempo(tempo_audio, sr)
    key_result = lexicon_key.detect_key(key_audio, 4400)
    energy = lexicon_energy.compute_energy(tempo_audio, sr, bpm_result["bpm"])
    waveform = lexicon_waveform.generate_waveform(waveform_audio, 12000)
    cues = lexicon_cues.detect_sections(bpm_result["beats"], tempo_audio, sr, bpm_result["bpm"])
```

**Verify:** `fourfour-analyze track.mp3 --backend lexicon_port --json` outputs valid JSON.

---

### Step 13: PythonStackBackend

**File:** `backends/python_stack.py`

**Optional deps:** `pip install -e ".[ml]"` (torch, librosa, deeprhythm)

**Algorithm sources:** `analysis-pipeline-handoff.md` §§1–6

- **BPM:** `DeepRhythmAnalyzer().analyze(path)` — 97% Acc2, ~0.2s/track
- **Key:** `librosa.chroma_cqt` + Krumhansl-Schmuckler — ~70%, convert to Camelot
- **Energy:** librosa feature fusion (spectral flux 30%, beat strength 25%, RMS 20%, centroid 15%, ZCR 10%)
- **Waveform:** numpy FFT, 3-band (low/mid/high), 2048-pt at full SR

**Verify:** `fourfour-analyze track.mp3 --backend python_deeprhythm --json` outputs valid JSON.

---

### Step 14: StratumDspBackend

**File:** `backends/stratum_dsp.py`

**Prerequisite:** Create `stratum-cli/` crate in workspace:
```
stratum-cli/
├── Cargo.toml   # depends on pioneer-usb-writer + stratum-dsp
└── src/main.rs  # reads path from argv, outputs JSON to stdout
```

Binary interface: `stratum-cli <audio_path>` → `{"bpm": 128.0, "key": "8A", "beats": [...], "version": "0.9.1"}`

Backend calls it via `subprocess.run([binary_path, track_path], capture_output=True)`.

**Verify:** `fourfour-analyze track.mp3 --backend stratum_dsp --json` outputs valid JSON (or clear "binary not found" error).

---

## Phase 3 — Benchmark Harness

*Produces: init → run → score CLI flow.*

---

### Step 15: Corpus builder

**File:** `manifest.py`

**CLI:** `fourfour-benchmark init ~/Music/corpus --name accuracy-v1`

Scans directory, extracts tags via mutagen, produces corpus JSON:

| Tag | ID3 frame | Vorbis comment | → Ground truth field |
|-----|-----------|----------------|---------------------|
| BPM | TBPM | BPM | `ground_truth.bpm` |
| Key | TKEY | INITIALKEY | `ground_truth.key` (→ Camelot) |
| Energy | TXXX:Energy | ENERGY | `ground_truth.energy` |
| Genre | TCON | GENRE | `genre` |

Missing tags → `null`. Tracks with no BPM/key tag are analyzed but not scored.

**Fingerprint:** SHA256(first 64KB + last 64KB + file size).

**Verify:** `fourfour-benchmark init /some/dir --name test` produces corpus JSON with populated ground_truth fields.

---

### Step 16: Ground truth extraction

**File:** `groundtruth.py`

Called by corpus builder — not a separate file format. Key functions:

- `extract_tags(path) → GroundTruth` — read mutagen tags → normalized types
- `_normalize_key(raw) → str | None` — "Abm" → "1A", "G#" → ..., etc.
- `load_corpus(path) → list[TrackEntry]` — load corpus JSON

**Verify:** `extract_tags()` reads BPM and key from a tagged MP3 correctly.

---

### Step 17: Comparison logic

**File:** `compare.py`

| Function | Returns |
|----------|---------|
| `compare_tempo(bpm_ours, bpm_gt)` | abs_delta, within_1pct, within_4pct, octave_error |
| `compare_key(key_ours, key_gt)` | exact, error_type (exact/relative/parallel/fifth/other) |
| `compare_energy(energy_ours, energy_gt)` | delta |
| `compare_beats(beats_ours, beats_gt, tol_ms=50)` | f_measure, median_offset_ms |

**Key error taxonomy (Camelot wheel):**
- **exact:** same code (8A == 8A)
- **relative:** same number, different letter (8A ↔ 8B)
- **parallel:** ±3 on same letter (8A ↔ 11A)
- **fifth:** adjacent on wheel (8A ↔ 7A or 8A ↔ 9A)
- **other:** everything else

**Verify:** `pytest tests/test_compare.py` passes with synthetic cases.

---

### Step 18: Run orchestrator + scoring

**Files:** `runner.py`, `scoring.py`

**CLI:**
```bash
fourfour-benchmark run \
    --corpus benchmark/manifests/accuracy-v1.corpus.json \
    --variants lexicon_port python_deeprhythm stratum_dsp \
    --parallel 4
```

**runner.py flow:**
1. Load corpus (manifest + embedded GT)
2. For each backend: analyze all tracks (cached), write `raw/{backend_id}.json`
3. Compare results vs GT, write `comparisons.json`
4. Aggregate scores, write `scoring.json`
5. Print summary table

**scoring.json output:**
```json
{
  "run_id": "run-20260421T120000Z",
  "backends": {
    "lexicon_port": {
      "bpm": { "acc1_pct": 82.0, "acc2_pct": 94.0, "octave_error_pct": 4.0 },
      "key": { "exact_match_pct": 64.0, "adjacent_match_pct": 80.0 },
      "energy": { "mean_delta": 1.2, "within_2_pct": 78.0 },
      "operational": { "mean_time_seconds": 0.8, "cache_hit_rate": 0.0 }
    }
  },
  "recommendation": "hybrid: DeepRhythm BPM + Lexicon key/energy/cues"
}
```

**Decision score formula:**
```python
score = (
    0.40 * bpm_acc2 +                                        # primary: BPM accuracy
    0.35 * key_exact +                                       # primary: key accuracy
    0.15 * (1 - min(mean_time / 5.0, 1.0)) +               # speed
    0.10 * (1 - min(dep_size_mb / 500, 1.0))               # dependency weight
)
```

**Verify:** Full benchmark run on 5 test tracks produces scoring.json.

---

## Phase 4 — CLI Polish

---

### Step 19: Wire all CLI commands

**File:** `cli.py` — two entry points, no shared dispatcher

**`fourfour-analyze`:**
```bash
fourfour-analyze <file> --backend lexicon_port --json
fourfour-analyze <file> --backend lexicon_port --backend python_deeprhythm  # compare two
fourfour-analyze <file>  # default: all available backends, table output
```

**`fourfour-benchmark`:**
```bash
fourfour-benchmark init <dir> --name <name>        # corpus from tagged files
fourfour-benchmark run --corpus <c> --variants ...  # analyze + score
fourfour-benchmark show <run_id>                    # display results
fourfour-benchmark compare <run1> <run2>            # before/after diff
fourfour-benchmark list                             # all runs
```

**Verify:** All commands work end-to-end.

---

### Step 20: Speed-only mode

```bash
fourfour-benchmark run --corpus <c> --speed-only --variants ...
```

Skips all comparison logic. Analyzes and times all tracks (even untagged). Output is operational metrics only: time per track, p95, cache hit rate, failure rate.

**Verify:** Run on ≥100 tracks produces timing table per backend.

---

## Dependency Graph

```
Step 1  (skeleton)
├── Step 2  (types) ──────────────────────────┐
├── Step 3  (config)                          │
├── Step 4  (audio_io) ──┐                    │
│   ├── Step 6  (lexicon_bpm) ──┐             │
│   ├── Step 7  (lexicon_key)   │ independent │
│   ├── Step 8  (lexicon_energy)│             │
│   ├── Step 9  (lexicon_waveform)            │
│   └── Step 10 (lexicon_cues) ─┘             │
│                                              │
├── Step 5  (cache) ─────────────┬────────────┤
│                                │            │
│           Step 11 (ABC) ───────┤            │
│           ├── Step 12 (LexiconPort)         │
│           ├── Step 13 (PythonStack)         │
│           └── Step 14 (StratumDsp)          │
│                                              │
│           Step 15 (manifest)                 │
│           Step 16 (groundtruth)              │
│           Step 17 (compare) ──────── Step 2 ─┘
│           Step 18 (runner + scoring)
│
│           Step 19 (CLI)
│           Step 20 (speed mode)
└──────────────────────────────────────────────
```

**Parallelizable:** Steps 6, 7, 8, 9 (any order) → Steps 12, 13, 14 (any order)

---

## Time Estimates

| Phase | Step | Time | Notes |
|-------|------|------|-------|
| 0 | 1. Skeleton | 30m | |
| 0 | 2. Types | 30m | Dataclasses, no logic |
| 0 | 3. Config | 20m | Path resolution |
| 0 | 4. Audio I/O | 2h | scipy filter chains, fixtures |
| 0 | 5. Cache | 30m | SHA1 + JSON |
| 1 | 6. Lexicon BPM | 4h | Most complex, ~200 lines |
| 1 | 7. Lexicon Key | 3h | Chroma + KS |
| 1 | 8. Lexicon Energy | 1.5h | Simplest |
| 1 | 9. Lexicon Waveform | 2h | FFT + bands |
| 1 | 10. Lexicon Cues | 3h | Phrase segmentation |
| 2 | 11. ABC + Registry | 1h | |
| 2 | 12. LexiconPort | 1h | Wires 6-10 |
| 2 | 13. PythonStack | 2h | DeepRhythm + librosa wrappers |
| 2 | 14. StratumDsp | 1h | Needs Rust CLI crate |
| 3 | 15. Manifest | 1h | Directory scan + fingerprint |
| 3 | 16. Ground truth | 30m | Tag extraction (inside corpus builder) |
| 3 | 17. Compare | 3h | BPM/key/energy diffs |
| 3 | 18. Runner + Scoring | 3h | Orchestration + aggregation |
| 4 | 19. CLI | 1.5h | All argparse commands |
| 4 | 20. Speed mode | 1h | |
| | **Total** | **~31h** | **~4 focused days** |

---

## Acceptance Criteria

The build is **done** when:

- [ ] `fourfour-analyze track.mp3 --backend lexicon_port --json` outputs BPM, key, energy, waveform, cue_points
- [ ] `fourfour-analyze track.mp3 --backend python_deeprhythm --json` outputs BPM, key, energy, waveform
- [ ] `fourfour-analyze track.mp3 --backend stratum_dsp --json` outputs BPM, key, beats
- [ ] `fourfour-benchmark init <dir> --name test` produces corpus JSON with ground_truth from tags
- [ ] `fourfour-benchmark run --corpus <c> --variants lexicon_port python_deeprhythm` produces scoring.json
- [ ] `fourfour-benchmark show <run_id>` prints comparison table
- [ ] All unit tests pass: `pytest tests/`
- [ ] Lexicon BPM detects within ±2 on click track fixtures
- [ ] Lexicon key detects correct key on chord fixtures
- [ ] scoring.json contains recommendation with decision scores

---

## Expected Outcomes → What To Ship

| Scenario | BPM result | Key result | Decision |
|----------|-----------|------------|----------|
| **A:** Lexicon matches Python | Competitive with 97% | ≥70% | Ship Lexicon (zero heavy deps) |
| **B:** DeepRhythm dominates BPM, Lexicon competitive on rest | Lexicon <90% | ≥65% | **Hybrid**: DeepRhythm BPM + Lexicon key/energy/cues |
| **C:** Python wins everything | Best on all metrics | Best | Ship Python stack |
| **D:** Close call | Within 5% | Within 5% | Run hybrid experiments, decide by weighted score |

**Outcome B is most likely** based on external benchmarking data. DeepRhythm's 97% Acc2 is hard to beat with onset+autocorrelation, but Lexicon's lightweight key/energy/cues approach may be good enough without torch.
