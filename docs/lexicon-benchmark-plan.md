# fourfour — Lexicon vs Python Analysis Benchmark Plan

> **Goal:** Port Lexicon's pure-JS analysis algorithms to Python, benchmark them head-to-head against our Python DSP stack (DeepRhythm + librosa + custom), and make a data-driven decision on which approach to ship.

---

## 1. Why This Matters

We have two viable paths for audio analysis in fourfour:

| Path | BPM | Key | Energy | Waveform | Phrase/Cues |
|------|-----|-----|--------|----------|-------------|
| **Python Stack** | DeepRhythm (97% Acc2, torch) | librosa chroma + KS (~70%) | librosa features | numpy FFT, 3-band | Not built yet |
| **Lexicon Approach** | Custom onset+autocorrelation (no deps) | Custom chroma + KS (no deps) | RMS + tempo + transient density | 128-pt FFT, 3-band, 12kHz | Energy-based phrase segmentation |

The Lexicon approach is **dependency-free** and runs at 12kHz/200Hz effective sample rates (dramatically faster). But we don't know its accuracy vs the Python ML stack. That's what this benchmark answers.

### Key Questions

1. **BPM accuracy:** Can Lexicon's onset+autocorrelation match DeepRhythm's 97% Acc2?
2. **Key accuracy:** Is Lexicon's custom chroma extraction any better/worse than librosa's?
3. **Speed:** How much faster is the Lexicon approach (downsampled to 200Hz/4.4kHz) vs the Python stack?
4. **Energy:** Which scoring correlates better with human judgment?
5. **Waveform:** Is the 128-point FFT at 12kHz sufficient, or do we need the Python 2048-point FFT?

---

## 2. Three Deliverables

### D1: `fourfour-analysis` Python CLI

The production analysis tool that runs our chosen Python stack on audio files.

```
analysis/
├── pyproject.toml
└── src/fourfour_analysis/
    ├── __init__.py
    ├── __main__.py
    ├── cli.py                    # fourfour-analyze, fourfour-benchmark commands
    ├── config.py
    ├── types.py                  # AnalysisResult, TrackEntry, GroundTruth, etc.
    ├── cache.py                  # content-addressed JSON cache
    ├── backends/
    │   ├── __init__.py
    │   ├── base.py               # AnalysisBackend ABC
    │   ├── registry.py           # ANALYSIS_VARIANTS dict
    │   ├── python_stack.py       # DeepRhythm + librosa + numpy (our chosen stack)
    │   ├── lexicon_port.py       # Port of Lexicon's algorithms (no heavy deps)
    │   └── stratum_dsp.py        # subprocess → Rust analyzer CLI
    ├── manifest.py               # Build/load track corpus manifest
    ├── groundtruth.py            # Load & validate ground truth
    ├── compare.py                # Diff analysis vs ground truth / vs each other
    ├── runner.py                 # Run orchestration
    └── analysis.py               # Aggregate metrics + recommendation
```

### D2: Lexicon Algorithm Port (`lexicon_port.py`)

A faithful Python reimplementation of Lexicon's 5 analysis functions, using only `numpy` + `scipy.signal`. No torch, no librosa, no ML.

```python
class LexiconPortBackend(AnalysisBackend):
    """Faithful port of Lexicon DJ's analysis algorithms.
    
    Uses the same approach:
    - 7-stage cascading lowpass via OfflineAudioContext equivalent (scipy.signal)
    - Onset detection + interval histogram + autocorrelation for BPM
    - Custom chroma via hand-rolled FFT + triangular kernel + KS for key
    - RMS + tempo + transient density for energy
    - 128-point FFT at 12kHz for waveform (3-band color)
    - Energy-based phrase segmentation for cue points
    """
```

### D3: Benchmark Harness

Reuses the samplebase pattern (manifest → run → compare → analyze) but adapted for audio analysis accuracy instead of retrieval quality.

---

## 3. Architecture

### AnalysisBackend ABC

```python
from abc import ABC, abstractmethod
from dataclasses import dataclass
from pathlib import Path

@dataclass(frozen=True)
class AnalysisResult:
    bpm: float | None
    key: str | None              # Camelot notation: "1A", "5B", etc.
    energy: int | None           # 1-10
    beats: list[float]           # timestamps in seconds
    waveform_peaks: list[tuple[float, float]]   # (min, max) per segment
    waveform_color: list[tuple[float, float, float]]  # (r, g, b) per segment
    cue_points: list[dict]       # [{type, time_seconds, label}]
    elapsed_seconds: float
    metadata: dict               # backend-specific info

@dataclass(frozen=True)  
class AnalysisRecord:
    track_id: str
    backend_id: str
    status: str                  # ok | failed | timeout
    result: AnalysisResult | None
    error: str | None

class AnalysisBackend(ABC):
    @abstractmethod
    def metadata(self) -> dict: ...
    
    @abstractmethod
    def analyze_track(self, track_path: str) -> AnalysisResult: ...
    
    def analyze_track_cached(self, track: TrackEntry) -> AnalysisRecord:
        # SHA1(content_fingerprint + backend_id + config_hash) → JSON cache
        ...
```

### Variant Registry

```python
ANALYSIS_VARIANTS = {
    # === Python ML stack (from samplebase benchmarking) ===
    "python_deeprhythm": {
        "backend": "python_stack",
        "label": "DeepRhythm + librosa KS",
        "bpm_engine": "deeprhythm",      # torch model, 97% Acc2
        "key_engine": "librosa_chroma",   # librosa chroma_cqt + KS
        "energy_engine": "librosa_features",
        "waveform_engine": "numpy_fft",
        "cue_engine": None,               # not implemented yet
        "network_required": False,
        "heavy_deps": ["torch"],
    },
    
    # === Lexicon port (dependency-light) ===
    "lexicon_port": {
        "backend": "lexicon_port",
        "label": "Lexicon algorithms (Python port)",
        "bpm_engine": "onset_autocorrelation",
        "key_engine": "custom_chroma_ks",
        "energy_engine": "rms_tempo_transient",
        "waveform_engine": "fft_128_3band",
        "cue_engine": "energy_phrase",
        "network_required": False,
        "heavy_deps": [],                 # numpy + scipy only
    },
    
    # === Stratum-dsp (Rust subprocess) ===
    "stratum_dsp": {
        "backend": "stratum_dsp",
        "label": "stratum-dsp (Rust)",
        "subprocess": True,
        "network_required": False,
        "heavy_deps": [],
    },
    
    # === Hybrids (future) ===
    "hybrid_deeprhythm_lexicon": {
        "backend": "python_stack",
        "label": "DeepRhythm BPM + Lexicon key/energy/cues",
        "bpm_engine": "deeprhythm",
        "key_engine": "custom_chroma_ks",    # from lexicon_port
        "energy_engine": "rms_tempo_transient",
        "waveform_engine": "fft_128_3band",
        "cue_engine": "energy_phrase",
    },
}
```

---

## 4. Lexicon Algorithm Port: Module Map

Each algorithm is a standalone Python function. Here's what gets ported from the reverse-engineering wiki:

### 4.1 BPM Detection (`lexicon_bpm.py`)

**Source:** Worker 182, Module 745 (~250 lines JS)

```python
def analyze_tempo(audio: np.ndarray, sample_rate: int, 
                  tempo_min: float = 80, tempo_max: float = 180) -> dict:
    """
    Lexicon's onset-detection + autocorrelation BPM.
    
    Pipeline:
    1. Compute spectral flux using sliding window
       window_size = (30 / tempo_max) * sample_rate
    2. Adaptive threshold: starts high, decrements by 5% of avg until enough peaks
    3. Interval histogram: pair peaks within 10 positions → BPM candidates
    4. Autocorrelation at 4, 8, 16 beat multiples
    5. Octave error resolution: test 1.5×, 0.67×, 2×, 0.5× candidates
    6. Genre heuristics: DnB ≈ 174, half-time detection for 85-90 BPM
    7. Fine-tune: snap to integer if correlation ≥ 95%
    """
```

**Python equivalents needed:**
- `scipy.signal` for filtering (replace `BiquadFilterNode` chain)
- `numpy` for spectral flux computation
- No external ML dependencies

**Key insight from Lexicon:** Input audio is pre-filtered to <200Hz (7-stage lowpass) BEFORE this function sees it. We need to replicate the preprocessing chain:

```python
def preprocess_for_tempo(audio: np.ndarray, sample_rate: int) -> np.ndarray:
    """7-stage cascading lowpass filter, matching Lexicon's OfflineAudioContext chain."""
    from scipy.signal import butter, sosfilt
    # 7 stages: 800→400→400→200→200→200→200 Hz
    frequencies = [800, 400, 400, 200, 200, 200, 200]
    filtered = audio
    for freq in frequencies:
        sos = butter(2, freq, btype='low', fs=sample_rate, output='sos')
        filtered = sosfilt(sos, filtered)
    return filtered
```

### 4.2 Key Detection (`lexicon_key.py`)

**Source:** Worker 182, functions O(), W(), E(), j(), C() (~300 lines JS)

```python
def detect_key(audio: np.ndarray, sample_rate: int) -> dict:
    """
    Lexicon's custom chroma + Krumhansl-Schmuckler.
    
    Pipeline:
    1. Slice: take 30%-60% of track
    2. FIR lowpass: 110 taps, cutoff ≈ 1999 Hz
    3. Decimate by ~10:1 → ~4400 Hz
    4. Frame: Blackman window, 16384 samples, 4096 hop
    5. FFT: 2048-point (hand-rolled Cooley-Tukey → use numpy.fft)
    6. Triangular kernel binning → 72 pitch classes (6 octaves × 12 semitones)
       bandwidth = 0.9 semitones (directional sparsity kernel)
    7. Average chroma: RMS-weighted mean across frames
    8. Krumhansl-Schmuckler: test 24 keys (12 major + 12 minor)
    9. Confidence: tanh(margin × normalized_strength)
    """
```

**Profiles (from Lexicon source):**
```python
MAJOR_PROFILE = [7.24, 3.50, 3.58, 2.85, 5.82, 4.56, 2.45, 6.99, 3.39, 4.56, 4.07, 4.46]
MINOR_PROFILE = [7.00, 3.14, 4.36, 5.40, 3.67, 4.09, 3.91, 6.20, 3.63, 2.87, 5.35, 3.83]
OCTAVE_WEIGHTS = [0.400, 0.556, 0.525, 0.608, 0.599, 0.491]
```

### 4.3 Energy Rating (`lexicon_energy.py`)

**Source:** Worker 182, function A() (~50 lines JS)

```python
def compute_energy(audio: np.ndarray, sample_rate: int, 
                   bpm: float, drop_regions: list | None = None) -> int:
    """
    Lexicon's 3-feature energy rating (1-10).
    
    Features:
    1. RMS energy (50% weight) - within drop regions or 30-70% of track
    2. Tempo factor (30% weight) - (bpm - 120) / 120
    3. Transient density (50% weight):
       - Segment into 0.014s windows
       - Count "strong beats": RMS rise > 0.2 AND RMS > 0.3
       - Normalize: clamp((density - 550000) / 150000, 0, 1)
    
    Penalty: if strong beats ≤ 200, score × 0.2 (flat/ambient tracks)
    """
```

### 4.4 Waveform Generation (`lexicon_waveform.py`)

**Source:** Worker 160 (~300 lines JS)

```python
def generate_waveform(audio: np.ndarray, sample_rate: int,
                      target_sample_rate: int = 12000,
                      fft_size: int = 128,
                      segment_width: int = 256) -> dict:
    """
    Lexicon's 3-band color waveform.
    
    Pipeline:
    1. Resample to 12kHz
    2. For each 256-sample window:
       - Min/max amplitude → waveform shape
       - 128-point FFT → spectrum
       - Low: 0-150 Hz, Mid: 150-1500 Hz, High: 1500+ Hz
       - Normalize to strongest band → RGB
       - Smooth with previous segment (50% blend)
    
    Returns:
        peaks: list of (min, max) tuples
        colors: list of (r, g, b) tuples
    """
```

### 4.5 Cue Point / Section Detection (`lexicon_cues.py`)

**Source:** Worker 182, function M() (~200 lines JS)

```python
def detect_sections(beats: list[float], audio: np.ndarray, 
                    sample_rate: int, bpm: float) -> list[dict]:
    """
    Lexicon's energy-based phrase segmentation.
    
    Pipeline:
    1. For each bar (4 beats):
       - RMS energy over bar duration
       - Beat strength = avg RMS at beat positions (±50ms)
       - Ramp type = linear regression slope (up/down/flat)
    2. Segment: split at mean energy → high/low sections
    3. Filter: min section = 64 beats, round to 4-bar boundaries
    4. Assign: Start=0, Drop=first high, Breakdown=first low after drop, etc.
    5. Emergency loop: find 16-beat stable section before last drop
    """
```

### 4.6 Preprocessing (`lexicon_preprocess.py`)

Shared preprocessing that matches Lexicon's OfflineAudioContext pipeline:

```python
def load_and_preprocess(path: str, target_sr: int = 44100) -> np.ndarray:
    """Load audio file to mono float32 at target sample rate."""
    import soundfile as sf
    data, sr = sf.read(path, dtype='float32', always_2d=True)
    mono = data.mean(axis=1)
    if sr != target_sr:
        from scipy.signal import resample
        mono = resample(mono, int(len(mono) * target_sr / sr))
    return mono, target_sr

def preprocess_for_tempo(audio: np.ndarray, sr: int) -> np.ndarray:
    """7-stage cascading lowpass → <200Hz content only."""
    ...

def preprocess_for_key(audio: np.ndarray, sr: int) -> np.ndarray:
    """FIR lowpass at 1999Hz + decimate to ~4400Hz."""
    ...

def preprocess_for_waveform(audio: np.ndarray, sr: int) -> np.ndarray:
    """Resample to 12kHz mono."""
    ...
```

---

## 5. Ground Truth Strategy

### Option A: Rekordbox master.db + ANLZ (existing plan)

From `benchmark-implementation-plan.md` — read BPM/key from `master.db`, beats from ANLZ files. Requires a Rekordbox USB export.

**Pros:** Direct comparison to Rekordbox (what DJs actually use).  
**Cons:** Need Rekordbox installed, need to curate + export a test collection. ANLZ reader still needs to be built in Rust.

### Option A2: Lexicon ground truth

Lexicon's output is already available — we can export analysis results from the app itself. But Lexicon isn't "ground truth" — it's a competing system.

### Option B: MTG-Jamendo dataset + manual annotation

Reuse the samplebase MTG-Jamendo corpus (500 tracks, 20 genres, 2.6GB, already downloaded). Add BPM/key annotations from a trusted source (e.g., MTG's own annotations, or manual tapping).

**Pros:** Already have the dataset. Covers 20 genres.  
**Cons:** MTG-Jamendo may not have reliable BPM/key annotations. Would need to create ground truth.

### Option C: Mixed approach (recommended for Phase 0)

1. **Curate a small corpus (~50 tracks)** from your actual DJ library (where you know the BPM/key)
2. **Cross-reference with Rekordbox** if available (for the tracks you already have analyzed)
3. **Use MTG-Jamendo as a speed/stress test** (500 tracks, just timing, no accuracy scoring)
4. **Manual verification** for accuracy: you listen + tap BPM, verify key on keyboard

This gives us:
- **Accuracy corpus:** 50 tracks with verified ground truth
- **Speed corpus:** 500 MTG-Jamendo tracks for raw performance numbers
- **Edge case corpus:** hand-picked tricky tracks (DnB, halftime, live drums, ambient)

### Ground Truth Schema

```json
{
    "schema_version": 1,
    "tracks": [
        {
            "id": "gt_001",
            "path": "/path/to/track.mp3",
            "title": "Track Name",
            "artist": "Artist",
            "genre": "House",
            "duration_seconds": 245.3,
            "bpm_groundtruth": 128.0,
            "bpm_source": "manual_tap",
            "key_groundtruth": "8A",
            "key_source": "keyboard_verification",
            "energy_groundtruth": 7,
            "energy_source": "subjective_rating",
            "notes": "steady 4/4, clear downbeat"
        }
    ]
}
```

---

## 6. Comparison Metrics

### 6.1 BPM Accuracy

| Metric | Definition | Lexicon | DeepRhythm | Target |
|--------|-----------|---------|------------|--------|
| **Acc1** | Within 1 BPM of ground truth | ? | ~90% | ≥ 90% |
| **Acc2** | Within 1 BPM or half/double | ? | ~97% | ≥ 95% |
| **Octave error rate** | BPM is 0.5× or 2× of truth | ? | ~3% | < 5% |
| **Median absolute error** | Median \|detected - truth\| | ? | ~0.3 | < 0.5 |

### 6.2 Key Accuracy

| Metric | Definition | Target |
|--------|-----------|--------|
| **Exact match** | Same Camelot code | ≥ 70% |
| **Adjacent** | ±1 on Camelot wheel | ≥ 85% |
| **Relative major/minor** | Same key, wrong mode (1A↔1B) | Track separately |

### 6.3 Energy Accuracy

| Metric | Definition | Target |
|--------|-----------|--------|
| **Correlation** | Pearson with human ratings | ≥ 0.7 |
| **Within ±2** | Score within 2 of ground truth | ≥ 80% |

### 6.4 Speed

| Metric | Target |
|--------|--------|
| **Mean time per track** | < 2 seconds |
| **P95 time per track** | < 5 seconds |
| **500 tracks total** | < 15 minutes |

### 6.5 Waveform Quality

Subjective — visual comparison of generated waveform vs Rekordbox export on CDJ display. Score: 1-5 rating per track.

### 6.6 Weighted Decision Score

```python
def decision_score(bpm_acc2, key_exact, speed_per_track, energy_corr, dep_size_mb):
    """
    Weighted score for final recommendation.
    
    BPM accuracy is most important (DJs live and die by it).
    Key accuracy is second (harmonic mixing).
    Speed matters for batch processing (10k+ tracks).
    Dependency size matters for deployment.
    """
    return (
        0.35 * bpm_acc2 +
        0.25 * key_exact +
        0.15 * (1 - min(speed_per_track / 5.0, 1.0)) +  # normalized speed
        0.10 * energy_corr +
        0.15 * (1 - min(dep_size_mb / 500, 1.0))         # normalized size
    )
```

---

## 7. Benchmark Corpus Design

### 7.1 Accuracy Corpus (50 tracks)

| Genre | Count | BPM Range | Why | Source |
|-------|-------|-----------|-----|--------|
| House | 8 | 120-130 | Steady 4/4, baseline accuracy | DJ library |
| Techno | 5 | 125-145 | Driving, minimal | DJ library |
| Drum & Bass | 5 | 170-175 | Half-time/octave errors | DJ library |
| Dubstep/Halftime | 4 | 70-75 / 140 | Half-time detection | DJ library |
| Hip-Hop | 5 | 80-100 | Swung, variable tempo | DJ library |
| Trance | 4 | 130-140 | Building, long intros | DJ library |
| Pop/EDM | 5 | 100-130 | Live drums, tempo changes | DJ library |
| Ambient/Downtempo | 4 | 70-110 | Low energy, sparse beats | DJ library |
| Breakbeat | 4 | 120-140 | Irregular rhythms | DJ library |
| Broken beat / 2-step | 3 | 120-135 | Syncopation | DJ library |
| Live drums / rock | 3 | 100-140 | Tempo drift | DJ library |

### 7.2 Speed Corpus (500 tracks)

The existing MTG-Jamendo dataset from samplebase. No accuracy scoring needed — just timing.

### 7.3 Edge Case Corpus (10 tracks)

Hand-picked problematic tracks:
- Half-time DnB that could be read as 85 or 170
- Tracks with long silent intros
- Tracks with tempo changes (dubstep → DnB)
- Ambient with no clear beat
- Tracks with very quiet / very loud mastering
- Acapellas
- Live recordings with drift

---

## 8. Build Order

### Week 1: D1 — Python CLI + D2 — Lexicon Port

```
Day 1-2: Scaffold + Python Stack Backend
├── Create analysis/ package structure
├── Implement AnalysisBackend ABC + types
├── Implement PythonStackBackend (DeepRhythm + librosa)
├── Implement manifest builder
└── Test on 5 tracks manually

Day 3-4: Lexicon Port
├── lexicon_preprocess.py (resample + filter chains)
├── lexicon_bpm.py (onset detection + autocorrelation)
├── lexicon_key.py (custom chroma + KS)
├── lexicon_energy.py (RMS + tempo + transient density)
├── lexicon_waveform.py (128-pt FFT, 3-band)
└── lexicon_cues.py (energy phrase segmentation)

Day 5: StratumDSP Backend + Integration
├── stratum_dsp.py (subprocess wrapper)
├── Wire up CLI: fourfour-benchmark init/run/analyze
└── Smoke test all 3 backends on 10 tracks
```

### Week 2: D3 — Benchmark Harness + Execution

```
Day 6: Ground Truth + Corpus
├── Curate 50-track accuracy corpus
├── Create ground truth JSON (manual BPM tap + key verification)
├── Build manifest from corpus
└── Verify MTG-Jamendo manifest for speed corpus

Day 7: Comparison Logic
├── compare.py: BPM diff, key match, energy correlation, beat F-measure
├── analysis.py: aggregate metrics, per-genre breakdown
└── Automated recommendation logic

Day 8: Accuracy Benchmark Run
├── Run all 3 backends on 50-track accuracy corpus
├── Run all 3 backends on 10-track edge case corpus
├── Generate comparison tables
└── Review results

Day 9: Speed Benchmark Run
├── Run all 3 backends on 500-track MTG-Jamendo corpus (timing only)
├── Generate speed comparison tables
└── Profile hotspots

Day 10: Decision + Report
├── Generate final recommendation
├── Write decision document
├── If hybrid wins: define which components from which backend
└── Update analysis-pipeline-handoff.md with results
```

---

## 9. Expected Outcomes & Decision Matrix

### Scenario A: Lexicon port matches Python stack (unlikely but possible)

→ **Ship Lexicon port.** Zero heavy dependencies. Fastest. Simplest deployment.

### Scenario B: DeepRhythm dominates BPM, Lexicon port is competitive on key/energy

→ **Hybrid.** Use DeepRhythm for BPM (97% Acc2), Lexicon port for key/energy/cues/waveform.
→ Still needs torch (~350MB) but only for BPM. Could explore ONNX conversion later.

### Scenario C: Python stack wins on everything

→ **Ship Python stack** as designed in `analysis-pipeline-handoff.md`.
→ Lexicon's approach informs optimization (12kHz resampling, 128-pt FFT for waveform).

### Scenario D: Close call

→ **Run hybrid experiments** (DeepRhythm BPM + Lexicon key/energy/cues).
→ Decision based on weighted score (§6.6).

---

## 10. CLI Commands

```bash
# Install
cd fourfour/analysis
python3 -m venv .venv && source .venv/bin/activate
pip install -e ".[all]"

# Single track analysis (any backend)
fourfour-analyze track.mp3 --backend python_deeprhythm --json
fourfour-analyze track.mp3 --backend lexicon_port --json
fourfour-analyze track.mp3 --backend stratum_dsp --json

# Batch analysis
fourfour-analyze ~/Music/corpus/ --backend lexicon_port --parallel 4 --output results/

# Benchmark: init corpus
fourfour-benchmark init ~/Music/benchmark-corpus --name accuracy-v1

# Benchmark: run all backends
fourfour-benchmark run \
    --manifest benchmark/manifests/accuracy-v1.manifest.json \
    --groundtruth benchmark/groundtruth/accuracy-v1.groundtruth.json \
    --variants python_deeprhythm lexicon_port stratum_dsp

# Benchmark: speed run (no accuracy, just timing)
fourfour-benchmark run \
    --manifest /path/to/mtg-jamendo.manifest.json \
    --speed-only \
    --variants python_deeprhythm lexicon_port stratum_dsp

# Benchmark: analyze results
fourfour-benchmark analyze run-20260421T120000Z

# Benchmark: compare two runs
fourfour-benchmark compare run-20260421T120000Z run-20260422T120000Z
```

---

## 11. Dependencies

### Python Stack Backend

```
torch          # ~349 MB (DeepRhythm dependency)
deeprhythm     # ~5 MB (BPM detection)
librosa        # ~4 MB (key, energy)
numpy          # ~20 MB (waveform, general)
soundfile      # ~2 MB (audio I/O)
scipy          # ~30 MB (signal processing)
mutagen        # ~1 MB (tags)
```

### Lexicon Port Backend

```
numpy          # ~20 MB (FFT, arrays)
scipy          # ~30 MB (signal processing: filters, resampling)
soundfile      # ~2 MB (audio I/O)
```

### StratumDSP Backend

```
# No Python deps — calls Rust binary via subprocess
# Requires: cargo build (symphonia, stratum-dsp, lofty)
```

---

## 12. File Sizes / Line Estimates

| File | Purpose | Est. Lines |
|------|---------|-----------|
| `src/fourfour_analysis/types.py` | Core data types | ~100 |
| `src/fourfour_analysis/cache.py` | Content-addressed JSON cache | ~80 |
| `src/fourfour_analysis/config.py` | Settings, path resolution | ~40 |
| `src/fourfour_analysis/backends/base.py` | AnalysisBackend ABC | ~60 |
| `src/fourfour_analysis/backends/python_stack.py` | DeepRhythm + librosa wrapper | ~150 |
| `src/fourfour_analysis/backends/lexicon_port.py` | Full Lexicon algorithm suite | ~600 |
| `src/fourfour_analysis/backends/lexicon_bpm.py` | BPM: onset + autocorrelation | ~200 |
| `src/fourfour_analysis/backends/lexicon_key.py` | Key: custom chroma + KS | ~200 |
| `src/fourfour_analysis/backends/lexicon_energy.py` | Energy: RMS + tempo + transient | ~80 |
| `src/fourfour_analysis/backends/lexicon_waveform.py` | Waveform: 128-pt FFT 3-band | ~150 |
| `src/fourfour_analysis/backends/lexicon_cues.py` | Cues: energy phrase segments | ~200 |
| `src/fourfour_analysis/backends/lexicon_preprocess.py` | Filter chains, resampling | ~100 |
| `src/fourfour_analysis/backends/stratum_dsp.py` | Rust subprocess wrapper | ~80 |
| `src/fourfour_analysis/backends/registry.py` | Variant definitions | ~60 |
| `src/fourfour_analysis/manifest.py` | Corpus manifest builder | ~80 |
| `src/fourfour_analysis/groundtruth.py` | Ground truth loader | ~60 |
| `src/fourfour_analysis/compare.py` | Diff + metrics | ~200 |
| `src/fourfour_analysis/runner.py` | Run orchestration | ~200 |
| `src/fourfour_analysis/analysis.py` | Aggregation + recommendation | ~120 |
| `src/fourfour_analysis/cli.py` | argparse CLI | ~150 |
| `pyproject.toml` | Package config | ~30 |

**Total: ~2,840 lines**

---

## 13. Key Decisions to Make Before Starting

1. **Ground truth source:** Manual annotation vs Rekordbox master.db vs both?
   → Recommend: Start with manual annotation on 50 tracks. Add Rekordbox later for beat-level ground truth.

2. **Lexicon port fidelity:** Exact 1:1 port or "inspired by" adaptation?
   → Recommend: 1:1 faithful port for the benchmark. We can optimize/adapt later.

3. **Python audio I/O:** `soundfile` (libsndfile) or `librosa.load` (audioread/soxr)?
   → Recommend: `soundfile` for WAV/FLAC, `librosa.load` as fallback for MP3. Lexicon port should use `soundfile` + `scipy.signal.resample` to avoid librosa dependency.

4. **Parallelism:** `multiprocessing.Pool` or `concurrent.futures.ProcessPoolExecutor`?
   → Recommend: `ProcessPoolExecutor` with configurable `max_workers`. Each backend gets its own pool.

5. **Should the Lexicon port use librosa for anything?**
   → No. The whole point is to test whether the dependency-free approach is viable. Use only `numpy` + `scipy` + `soundfile`.
