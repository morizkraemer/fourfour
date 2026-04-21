# DJ Audio Analysis Stack — Open Source Reference

A complete repo map with 2 options per analysis layer, integration guidance, and known pain points for building an open-source Rekordbox competitor.

---

## 1. BPM / Tempo Estimation

### Option A: Essentia — TempoCNN

| | |
|---|---|
| **Repo** | [github.com/MTG/essentia](https://github.com/MTG/essentia) |
| **Stars** | ~3.4k |
| **Language** | C++ core, Python bindings |
| **License** | AGPL v3 |
| **Install** | `pip install essentia-tensorflow` |
| **Key algorithm** | `TempoCNN` — CNN trained on mel spectrograms, outputs global BPM + local BPM per ~6s segment with confidence scores |
| **Also provides** | Waveform generation, spectral features, loudness, HPCP, onset detection — the "everything" toolkit |

**Strengths:** Fastest path to global BPM. TempoCNN handles tempo changes via local estimations. The broader Essentia ecosystem means you get dozens of other features (energy, loudness, spectral centroid) from the same pipeline without adding dependencies.

**Pain points:**
- AGPL license is viral — if your app links against Essentia, the entire app must be AGPL. This is the #1 architectural constraint.
- The TensorFlow dependency is heavy (~400MB+). The `essentia-tensorflow` pip package only works on Linux x86_64 currently.
- TempoCNN does BPM estimation, not beat *position* tracking. You get "this track is 128 BPM" but not "the beats land at these exact timestamps." You still need a separate beat tracker for grid construction.
- Building from source on macOS/Windows is notoriously painful. Docker is the recommended path.
- The API has two paradigms (standard vs streaming mode) that are confusing for newcomers.

### Option B: Madmom — RNNBeatProcessor + DBNBeatTrackingProcessor

| | |
|---|---|
| **Repo** | [github.com/CPJKU/madmom](https://github.com/CPJKU/madmom) |
| **Stars** | ~1.4k |
| **Language** | Python (with Cython extensions) |
| **License** | BSD-like (custom academic license) |
| **Install** | `pip install madmom` |
| **Key algorithm** | `DBNBeatTracker` — RNN activation + Dynamic Bayesian Network for beat/downbeat positions |

**Strengths:** Best-in-class beat position accuracy. Research consistently shows it outperforms librosa and other traditional methods for locating exact beat timestamps. The DBN inference model handles tempo changes gracefully. Includes downbeat tracking (bar-level), which is essential for phrase-aligned beatgrids.

**Pain points:**
- Python 3.10+ compatibility has been a persistent issue. The Cython extensions occasionally break with new Python/NumPy versions. Check open issues before upgrading.
- No real-time / streaming mode — it's offline-only. You must process the entire track before getting results.
- The pre-trained models are baked into the package and can't be easily retrained or fine-tuned on your own data.
- Slower than Essentia's TempoCNN for pure BPM estimation — madmom does more work (full beat position tracking) which takes ~0.3-0.5x realtime on CPU.
- The project is maintained but not actively developed — updates are infrequent.

### How they work together

Use **Essentia TempoCNN** for the quick global BPM number (fast, gives you a number to display immediately). Then run **madmom DBNBeatTracker** for the actual beat positions array that you use to construct the beatgrid. The TempoCNN result can inform madmom's tempo range parameter, avoiding octave errors (e.g., 64 vs 128 BPM).

```python
# Essentia: fast global BPM
import essentia.standard as es
audio = es.MonoLoader(filename='track.wav', sampleRate=11025)()
global_bpm, local_bpm, local_probs = es.TempoCNN(
    graphFilename='deeptemp-k16-3.pb'
)(audio)

# Madmom: precise beat positions using the BPM as a hint
import madmom
proc = madmom.features.beats.RNNBeatProcessor()
act = proc('track.wav')
beat_proc = madmom.features.beats.DBNBeatTrackingProcessor(fps=100)
beats = beat_proc(act)  # array of beat timestamps in seconds
```

**Integration pain point:** Essentia expects 11025 Hz audio for TempoCNN, madmom expects 44100 Hz. You'll need to load audio twice or resample. Also, Essentia's AGPL license infects everything it touches — if you want a permissive license for your app, you'll need to run Essentia as a separate subprocess/service.

---

## 2. Beat Grid Construction & Downbeat Tracking

### Option A: Madmom — DBNDownBeatTracker

| | |
|---|---|
| **Repo** | [github.com/CPJKU/madmom](https://github.com/CPJKU/madmom) |
| **Same repo as above** | Uses `RNNDownBeatProcessor` + `DBNDownBeatTrackingProcessor` |
| **Key algorithm** | Joint beat + downbeat tracking via RNN activations and bar-position-aware HMM |

**Strengths:** Gives you both beat AND downbeat (bar boundary) positions. This is critical for beatgrid alignment — you need to know where bar 1 starts, not just where beats are. The DBN models tempo and meter jointly, handling 3/4 vs 4/4 correctly.

**Pain points:**
- Downbeat accuracy is inherently lower than beat accuracy — identifying the "one" is harder than finding beats in general.
- The meter detection (3/4 vs 4/4 vs 6/8) is not always reliable on electronic music with minimal harmonic structure.
- No dynamic beatgrid support out of the box — if a track changes tempo, you need to segment it yourself and run the tracker per-segment.

### Option B: BeatNet

| | |
|---|---|
| **Repo** | [github.com/mjhydri/BeatNet](https://github.com/mjhydri/BeatNet) |
| **Stars** | ~500 |
| **Language** | Python (PyTorch) |
| **License** | MIT |
| **Install** | `pip install BeatNet` |
| **Key algorithm** | CRNN + particle filtering for joint beat/downbeat/tempo/meter |
| **Also see** | [github.com/mjhydri/BeatNet-Plus](https://github.com/mjhydri/BeatNet-Plus) — improved training, newer weights |
| **Also see** | [github.com/mjhydri/1D-StateSpace](https://github.com/mjhydri/1D-StateSpace) — compact state space variant |

**Strengths:** MIT license (huge advantage over madmom's academic license). Four operating modes including real-time streaming from microphone. Uses BeatNet's own CRNN neural network but madmom's DBN for offline inference — getting the best of both. Joint beat/downbeat/tempo/meter in one pass.

**Pain points:**
- Depends on BOTH librosa AND madmom as prerequisites, so you inherit all their dependency issues.
- The streaming/real-time modes require PyAudio, which is notoriously hard to install cross-platform.
- ~47 open issues, many unresolved, and the author's response time is slow.
- Documentation is sparse — you'll be reading the paper and source code.
- The particle filtering mode can produce unstable results in the first few seconds of a track.

### How they work together

BeatNet actually wraps madmom's DBN internally for offline mode, so running both is redundant for offline analysis. Choose one:
- **BeatNet** if you need real-time capability or want MIT licensing
- **Madmom directly** if you want the most battle-tested offline results and don't need streaming

For dynamic beatgrids (Rekordbox-style tempo change handling), you'll need to build your own segmentation layer on top of either tool, detecting tempo transition points and constructing grid zones.

---

## 3. Key Detection

### Current Decision: Essentia KeyExtractor `bgate`

An in-repo benchmark against the Beatport EDM Key Dataset found `essentia_key_bgate` is the best current practical backend for this project.

| System | Ground truth | Exact | Exact + adjacent |
|---|---|---:|---:|
| Rekordbox | Beatport labels | 47% | 55% |
| Essentia `bgate` | Beatport labels | 54.0% | 68.9% |

The Essentia result used the 598-track clean single-key subset. That is the right target for a one-key Camelot detector. See [`key-detection-benchmark-findings.md`](./key-detection-benchmark-findings.md).

Use the `essentia_key_bgate` variant for key-only benchmarking and as the current production candidate. Keep `essentia_key_edmm` available as an alternate because it scored the highest exact-or-adjacent rate.

### Option A: Essentia Key Algorithm (HPCP-based)

| | |
|---|---|
| **Repo** | [github.com/MTG/essentia](https://github.com/MTG/essentia) — same repo as BPM |
| **Key algorithm** | `KeyExtractor` — Harmonic Pitch Class Profile (HPCP) + key profile matching |
| **Chosen profile** | `bgate` |
| **Also benchmarked** | `edma`, `edmm`, `shaath`, `krumhansl`, `temperley` |

**Strengths:** Best measured result in the current project benchmark. Fast enough for batch import. Actively maintained upstream. Simple Python binding and no TensorFlow dependency for `KeyExtractor`.

**Pain points:**
- Exact accuracy is still only 54.0% on the clean Beatport subset, so manual spot checks remain necessary.
- Major/minor and fifth-adjacent errors still happen.
- License must be reviewed before deciding whether Essentia can be linked directly or should remain isolated as a sidecar process.

### Option B: OpenKeyScan / CNN key detection

CNN key detection may still be the future upgrade path, but do not choose it from claims alone. Before adding it, verify repository health, installability on supported Python versions, model licensing, binary size, and score it against the same Beatport corpus.

`key-cnn` was not accepted as a project dependency because it is not on PyPI and appears stale with old Python/TensorFlow assumptions.

---

## 4. Phrase / Structure Analysis

### Option A: MSAF (Music Structure Analysis Framework)

| | |
|---|---|
| **Repo** | [github.com/urinieto/msaf](https://github.com/urinieto/msaf) |
| **Stars** | ~500 |
| **Language** | Python |
| **License** | MIT |
| **Install** | `pip install msaf` |
| **Key algorithms** | Foote novelty, spectral clustering (scluster), OLDA, checkerboard kernel — multiple boundary + labeling algorithms |

**Strengths:** The only dedicated framework for music structure analysis. Implements multiple segmentation algorithms with a unified API. Includes evaluation metrics against ground truth annotations. Can use pre-computed beat positions from madmom/BeatNet as input, improving boundary precision.

**Pain points:**
- Last release was v0.1.6 — the project is largely unmaintained. Expect dependency conflicts with modern Python/NumPy.
- The algorithms were designed for academic evaluation, not DJ use cases. You get boundaries labeled by acoustic similarity (A-B-A-C structure), not semantic labels ("intro", "verse", "chorus", "drop").
- Getting DJ-meaningful labels (intro/breakdown/drop/outro) requires significant post-processing on top of MSAF's raw boundaries.
- Depends on librosa and can be slow on long tracks.
- Documentation is thin — the Jupyter notebook is your best guide.

### Option B: Custom pipeline using Essentia features + self-similarity

| | |
|---|---|
| **Repo** | [github.com/MTG/essentia](https://github.com/MTG/essentia) for features |
| **Approach** | Compute MFCCs or chroma per beat-synchronous frame → build self-similarity matrix → apply novelty detection for boundaries |
| **Reference paper** | Barwise segmentation (arxiv 2311.18604) — open source, competitive with supervised methods |

**Strengths:** Full control over the pipeline. Can tune specifically for DJ music (EDM, house, techno have different structural patterns than pop). Using beat-synchronous features (one feature vector per beat instead of per frame) naturally aligns boundaries to the beatgrid.

**Pain points:**
- You're building from scratch — no turnkey solution.
- Self-similarity segmentation works well for finding boundaries but gives you unlabeled segments (just "this section is similar to that section").
- Adding semantic labels (chorus, verse, drop) requires either a classifier trained on labeled DJ music or heuristic rules (e.g., "loudest section = drop").
- Needs significant experimentation to tune parameters per genre.

### How they work together

Use **MSAF** as a starting point to understand the algorithms, then **build a custom pipeline** using Essentia features tuned for your target genres. Feed beat positions from madmom into the feature computation to get beat-synchronous representations. Apply MSAF's Foote novelty detection for boundaries, then build a lightweight classifier for labeling sections.

---

## 5. Audio Embeddings / Similarity

### Option A: LAION CLAP

| | |
|---|---|
| **Repo** | [github.com/LAION-AI/CLAP](https://github.com/LAION-AI/CLAP) |
| **Stars** | ~3k |
| **Language** | Python (PyTorch) |
| **License** | Apache 2.0 |
| **Install** | `pip install laion-clap` |
| **Also on** | [HuggingFace: laion/clap-htsat-unfused](https://huggingface.co/laion/clap-htsat-unfused) |
| **Embedding dim** | 512 |

**Strengths:** The de facto standard for audio-text embeddings. Produces 512-dim vectors that enable both audio-to-audio similarity and text-to-audio search ("dark minimal techno"). Trained on LAION-Audio-630K (630k audio-text pairs). Strong perceptual alignment — similarity scores meaningfully correspond to how "similar" two tracks sound. Apache 2.0 license. Already used in published DJ research (Zero-Shot Crate Digging paper).

**Pain points:**
- The audio encoder was trained on 7-second windows at 48 kHz. Full tracks need to be windowed and mean-pooled (typically 3 windows at 10%, 45%, 80% of track duration).
- The ~5MB model download on first load. Inference is not trivially fast — expect ~0.5-1s per track for embedding generation on CPU.
- Text-to-audio search requires careful prompt engineering. "Energetic house music" works better than "banging tune."
- Cosine similarity scores are relative, not absolute — 0.8 doesn't inherently mean "very similar" across all domains. You'll need to calibrate thresholds for your specific library.
- The model is frozen — you can't fine-tune on your DJ's specific taste without training a new model.

### Option B: Microsoft CLAP

| | |
|---|---|
| **Repo** | [github.com/microsoft/CLAP](https://github.com/microsoft/CLAP) |
| **Stars** | ~800 |
| **Language** | Python (PyTorch) |
| **License** | MIT |
| **Install** | `pip install msclap` |
| **Embedding dim** | 1024 |
| **Versions** | `2022`, `2023`, `clapcap` (audio captioning) |

**Strengths:** MIT license (vs LAION's Apache 2.0 — both permissive). The `clapcap` model can generate text captions for audio, which is unique — "this is an energetic electronic dance track with a strong 4/4 beat." Higher-dimensional embeddings (1024 vs 512) may capture more nuance. The 2023 version was evaluated on 26 downstream tasks.

**Pain points:**
- Less community adoption than LAION CLAP for music-specific tasks.
- The GTZAN genre classification performance varies between versions (51% to 71%), suggesting music isn't its strongest domain.
- Fewer pre-trained checkpoints specifically tuned for music.
- API is slightly different from LAION CLAP, and documentation is sparser.

### How they work together

Use **LAION CLAP** as your primary embedding model for track similarity and text search. Store the 512-dim vector per track in a vector database (FAISS, Milvus, or even SQLite with a custom distance function). Use **Microsoft CLAP's clapcap** model to auto-generate text descriptions of tracks for display/search enrichment. Both can run as background processing jobs during library import.

---

## 6. Source Separation (Stems)

### Option A: Demucs v4

| | |
|---|---|
| **Repo** | [github.com/adefossez/demucs](https://github.com/adefossez/demucs) (maintained fork) |
| **Original** | [github.com/facebookresearch/demucs](https://github.com/facebookresearch/demucs) (archived) |
| **Stars** | ~8k (original) |
| **Language** | Python (PyTorch) |
| **License** | MIT |
| **Models** | `htdemucs` (4 stems), `htdemucs_6s` (6 stems: +guitar, piano), `htdemucs_ft` (fine-tuned, best quality) |
| **SDR** | 9.20 dB on MUSDB HQ (state of the art) |

**Strengths:** State-of-the-art separation quality. MIT license. Hybrid Transformer architecture works in both time and frequency domains. The fine-tuned model produces remarkably clean stems. 6-stem model adds guitar and piano separation. Well-documented, large community.

**Pain points:**
- The original facebookresearch repo is archived/unmaintained since the creator left Meta. The fork at adefossez/demucs is the active one but explicitly states "not actively maintained anymore."
- GPU memory hungry — the fine-tuned model needs ~6GB VRAM for a 5-minute track. CPU inference is very slow (10-30x realtime).
- Processing time is significant even on GPU (~30s for a 5-min track with htdemucs, ~2min with htdemucs_ft).
- The 6-stem model's guitar and piano separation quality is noticeably worse than the core 4 stems.

### Option B: Demucs-rs (Rust implementation)

| | |
|---|---|
| **Repo** | [github.com/nikhilunni/demucs-rs](https://github.com/nikhilunni/demucs-rs) |
| **Language** | Rust (using Burn ML framework) |
| **License** | Check repo |
| **Outputs** | VST3/CLAP plugin, CLI, WebAssembly browser app |
| **GPU** | Metal (macOS), Vulkan (Linux/Windows), WebGPU (browser) |

**Strengths:** Native performance without Python overhead. Ships as a DAW plugin (VST3/CLAP) with per-stem aux outputs. Browser version via WebAssembly runs 100% locally. Uses the same HTDemucs model weights (auto-downloaded from HuggingFace). No Python dependency for your end-user application.

**Pain points:**
- Newer/less mature than the Python original.
- WebAssembly build is significantly slower than native CLI.
- The DAW plugin is macOS-only currently.
- Rust compilation and Burn framework add complexity to the build process.

### How they work together

Use **Demucs (Python)** for your analysis pipeline / backend processing and **demucs-rs** if you need to ship a native application or want browser-based separation. Both use the same model weights and produce identical results.

---

## 7. Waveform Generation & Audio I/O

### Option A: Essentia (already in your stack)

Essentia's `MonoLoader` / `AudioLoader` handle virtually every audio format via FFmpeg/LibAV. The `Waveform` and `Envelope` algorithms generate the display data. Since you're likely already using Essentia for other features, this avoids adding another dependency.

### Option B: Aubio

| | |
|---|---|
| **Repo** | [github.com/aubio/aubio](https://github.com/aubio/aubio) |
| **Stars** | ~3.3k |
| **Language** | C with Python bindings |
| **License** | GPL v3 |
| **Install** | `pip install aubio` |

**Strengths:** Extremely lightweight and fast. Designed for real-time / low-latency. Good onset detection for supplementing beat tracking. The C core makes it ideal for embedding in non-Python applications.

**Pain points:**
- GPL v3 license (less restrictive than AGPL but still copyleft).
- The Python package (`aubio 0.4.9`) hasn't been updated in years.
- Feature set is narrower than Essentia — onset, pitch, beat, tempo, MFCCs but no key detection, no ML models.

---

## Putting It All Together — Recommended Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    IMPORT PIPELINE                       │
│                                                         │
│  Audio File                                             │
│     │                                                   │
│     ├──▶ Essentia MonoLoader (audio I/O)                │
│     │                                                   │
│     ├──▶ Essentia TempoCNN ──▶ Global BPM (fast)       │
│     │                                                   │
│     ├──▶ Madmom DBNBeatTracker ──▶ Beat positions      │
│     │    └──▶ DBNDownBeatTracker ──▶ Downbeat/bar      │
│     │         └──▶ Beatgrid construction                │
│     │                                                   │
│     ├──▶ OpenKeyScan CNN ──▶ Musical key               │
│     │                                                   │
│     ├──▶ Essentia descriptors ──▶ Energy, loudness,    │
│     │                              spectral features    │
│     │                                                   │
│     └──▶ Essentia Waveform ──▶ Display waveform        │
│                                                         │
├─────────────────────────────────────────────────────────┤
│               BACKGROUND PROCESSING                     │
│                                                         │
│     ├──▶ LAION CLAP ──▶ 512-dim embedding              │
│     │    └──▶ Store in vector DB for similarity search  │
│     │                                                   │
│     ├──▶ MSAF / Custom segmentation ──▶ Phrase markers │
│     │                                                   │
│     └──▶ Demucs v4 ──▶ Stem separation (optional)      │
│                                                         │
├─────────────────────────────────────────────────────────┤
│                 NOVEL FEATURES                          │
│                                                         │
│     ├──▶ CLAP text search: "dark minimal techno"       │
│     ├──▶ CLAP audio search: "find tracks like this"    │
│     └──▶ MS CLAP clapcap: auto-generate descriptions   │
└─────────────────────────────────────────────────────────┘
```

## Critical Cross-Cutting Pain Points

### 1. License incompatibility
Essentia (AGPL) cannot be linked with MIT/Apache code without the whole project becoming AGPL. **Mitigation:** Run Essentia as a separate subprocess or microservice communicating via JSON/protobuf. This keeps the AGPL "infection" contained.

### 2. Python dependency hell
Madmom, BeatNet, CLAP, and Essentia all depend on different versions of NumPy, SciPy, and PyTorch/TensorFlow. **Mitigation:** Use separate virtual environments or Docker containers per analysis module, or pin exact versions aggressively.

### 3. Sample rate mismatch
Essentia TempoCNN wants 11025 Hz. Madmom wants 44100 Hz. CLAP wants 48000 Hz. Demucs works at the file's native rate. **Mitigation:** Load audio once at the highest rate (48000 Hz) and resample per-tool. Use librosa.resample or Essentia's MonoLoader with sampleRate parameter.

### 4. Processing time budget
Full analysis (BPM + beats + key + embeddings + structure + stems) takes 30-60s per track on a modern CPU. **Mitigation:** Parallelize by module. BPM/beats/key can run concurrently since they read the same audio independently. Stems should be optional/on-demand.

### 5. Octave/half-time errors
The most common BPM error across all tools: detecting 64 instead of 128, or 170 instead of 85. **Mitigation:** Use the TempoCNN global BPM to constrain madmom's tempo range. Cross-validate between tools. Apply genre-aware priors (house is 120-135, drum & bass is 170-180).

### 6. No single tool matches Rekordbox's phrase analysis
Rekordbox's AI-powered phrase analysis (auto-labeling intro/verse/chorus/drop/outro) has no open-source equivalent. This is the biggest gap. **Mitigation:** Combine MSAF boundaries with CLAP embeddings to cluster and label sections. Train a simple classifier on a small labeled dataset of DJ tracks.
