# Lexicon DJ — Technical Deep Dive

> Reverse-engineered from Lexicon v1.10.7 (`com.rekord.cloud.lexicon`), an Electron app by Rekordcloud.

---

## Table of Contents

1. [Stack Overview](#1-stack-overview)
2. [Why It's Fast](#2-why-its-fast)
3. [Audio Analysis Pipeline](#3-audio-analysis-pipeline)
4. [BPM Detection](#4-bpm-detection)
5. [Key Detection](#5-key-detection)
6. [Beat Grid & First Beat](#6-beat-grid--first-beat)
7. [Cue Points & Section Detection](#7-cue-points--section-detection)
8. [Energy Rating](#8-energy-rating)
9. [Waveform Generation](#9-waveform-generation)
10. [Waveform UI: Scrolling View](#10-waveform-ui-scrolling-view)
11. [Database & Storage](#11-database--storage)
12. [Worker Architecture](#12-worker-architecture)
13. [Optimization Patterns](#13-optimization-patterns)
14. [What's Not Local](#14-whats-not-local)

---

## 1. Stack Overview

### Runtime

| Layer | Technology | Purpose |
|-------|-----------|---------|
| Shell | **Electron** (Chromium + Node.js) | Desktop app container |
| UI | **Webix** (datatable/tree) + **vanilla JS** | Data tables, layout, tree views |
| Player | **Web Audio API** (`AudioContext`) | Playback, filtering, decoding |
| Analysis | **Web Workers** (5 separate JS files) | BPM, key, energy, cue points, waveform |
| Database | **better-sqlite3** (plain SQLite, no encryption) | All track data, playlists, settings |
| Audio I/O | **ffmpeg/ffprobe** (bundled binary) | File splitting, format conversion |
| Tag R/W | **lexicon-tagger** (Python 3.12 + eyed3) | ID3 tag read/write only |
| Encoding | **lame 3.100** (bundled binary) | MP3 re-encoding during sync |

### What's NOT in the stack

- **No ML / neural networks** — all analysis is traditional DSP
- **No WASM** — pure JavaScript
- **No audio analysis npm packages** — custom implementations
- **No SQLCipher** — plain SQLite database
- **No React/Vue/Svelte** — vanilla JS + Webix

### File Layout

```
Lexicon.app/Contents/Resources/
├── app.asar                          ← main application code
│   ├── main/index.js                 ← Electron main process (31K lines)
│   └── renderer/main_window/
│       ├── index.js                  ← UI + all business logic (11MB)
│       ├── 182.index.worker.js       ← analysis worker (624KB)
│       ├── 160.index.worker.js       ← waveform worker (38KB)
│       ├── 940.index.worker.js       ← utility worker (100KB)
│       ├── 639.index.worker.js       ← lodash + helpers (155KB)
│       ├── 468.index.worker.js       ← utility (8KB)
│       ├── 110.index.worker.js       ← utility (11KB)
│       └── *.svg, *.png, *.woff2     ← assets
└── app.asar.unpacked/lib/
    ├── ffmpeg/ffmpeg, ffprobe         ← audio tools
    ├── lame/lame                      ← MP3 encoder
    └── lexicon-tagger/               ← Python ID3 tagger
```

---

## 2. Why It's Fast

Lexicon analyzes tracks dramatically faster than Rekordbox. Here's exactly why:

### 2.1 Aggressive Data Reduction Before Analysis

A 6-minute stereo 44.1kHz track = **~31.7 million samples**. Lexicon reduces this:

| Analysis | What it receives | Effective data | Reduction |
|----------|-----------------|----------------|-----------|
| **BPM / Energy / Cues** | 7-stage 200Hz lowpass filter | ~720K samples | **44× less** |
| **Key detection** | 1975Hz lowpass + 4:1 downsample, 30% slice | ~79K samples | **400× less** |
| **Waveform** | Resampled to 12kHz mono | ~4.3M samples | **7× less** |

Rekordbox likely processes at or near the full 44.1kHz sample rate for everything. That alone makes Lexicon 10-100× faster per analysis step.

### 2.2 The Preprocessing is a Single Offline Render Pass

The 7-stage cascading lowpass filter chain runs via `OfflineAudioContext.startRendering()` — this is a **single synchronous-feeling render** that's heavily optimized by the browser's native audio engine (CoreAudio on macOS). It's not iterating sample-by-sample in JavaScript.

```
800Hz → 400Hz → 400Hz → 200Hz → 200Hz → 200Hz → 200Hz
```

Seven `BiquadFilterNode` stages = -84dB/oct slope, eliminating everything above ~200Hz. The result contains only kick drums and bass — exactly what you need for tempo detection.

### 2.3 Worker Pool with Zero-Copy Transfers

```javascript
// Pre-spawn N workers (one per track up to a limit)
// Send data as Transferable ArrayBuffers — zero-copy
// Each worker processes independently
this.threadManager = new ThreadManager2(concurrency, workerFactory)
```

`Transferable` ArrayBuffers transfer ownership between threads without copying. For a 6-minute track at 200Hz filtered, that's only ~2.9MB of Float32 data — trivially fast to transfer.

### 2.4 Tiny FFT Sizes

The waveform uses a **128-point FFT**. That's nearly free. Compare to a typical 2048 or 4096 point FFT in professional audio software — Lexicon does 16-32× less work per FFT.

### 2.5 Simple Algorithms, No Over-Engineering

| Algorithm | Complexity | Implementation |
|-----------|-----------|---------------|
| BPM | O(N × peaks²) | Onset detection + interval histogram + autocorrelation |
| Key | O(frames × 72 notes) | Standard Krumhansl-Schmuckler |
| Energy | O(N) | Three weighted features |
| Cue points | O(bars) | Energy threshold segmentation |
| Waveform | O(N / 256) | 128-point FFT per segment |

No neural networks, no spectral fingerprinting, no CQT, no deep learning. Traditional DSP that runs in microseconds per sample on V8's JIT compiler.

### 2.6 V8 JIT Compiler

JavaScript on V8 is not slow. The `Float32Array` operations in the analysis workers compile to near-native machine code via TurboFan (V8's optimizing compiler). The hot loops in onset detection, autocorrelation, and FFT are all tight numeric kernels — exactly what JIT compilers excel at.

---

## 3. Audio Analysis Pipeline

### Orchestration Flow

```
User clicks "Analyze"
        │
        ▼
AudioAnalyzer.start(trackIds[])
        │
        ├─ Calculate concurrency = clamp(trackCount)
        ├─ Spawn ThreadManager2 with N workers
        │
        ▼
For each track (parallel across workers):
        │
        ├─ 1. Decode audio via OfflineAudioContext
        │     Input: file bytes → AudioBuffer (44.1kHz stereo)
        │
        ├─ 2. Preprocess THREE paths simultaneously:
        │     ├─ Path A: Full track → 7× LPF → mono (tempo/beatgrid)
        │     ├─ Path B: Full track → 7× LPF → mono (energy/cues) [same data]
        │     └─ Path C: 30%-60% slice → raw mono (key)
        │
        ├─ 3. Send to Worker 182 as 5 Transferable ArrayBuffers:
        │     [0] = settings
        │     [1] = sampleRate
        │     [2] = filtered audio (for energy/cues)
        │     [3] = filtered audio (for tempo/beatgrid)
        │     [4] = raw audio (for key)
        │
        ├─ 4. Worker 182 processes:
        │     ├─ analyzeTempo() → BPM, confidence, peaks
        │     ├─ Wi() → first beat, beat grid
        │     ├─ M() → section points (drops, breakdowns)
        │     ├─ A() → energy (1-10)
        │     └─ O() → key (C, Am, etc.)
        │
        ├─ 5. Waveform via separate Worker 160:
        │     ├─ ffmpeg splits audio into 5-second WAV segments
        │     ├─ Each segment → OfflineAudioContext resample to 12kHz
        │     ├─ Worker 160: 128-point FFT → 3-band color → min/max/color
        │     ├─ Overview → OffscreenCanvas → WebP blob
        │     └─ Zoom data → per-segment Float32Arrays
        │
        └─ 6. Save results to SQLite:
              ├─ Track table: bpm, key, energy
              ├─ Tempomarker table: {startTime, bpm} beat grid
              ├─ Cuepoint table: auto cue points
              └─ Waveform table: WebP overview + previewCues JSON
```

### Key Decision: Separate Waveform Worker

The waveform is generated by **Worker 160** (not Worker 182). This means waveform generation can run in parallel with analysis. The audio is loaded incrementally — ffmpeg streams 5-second WAV segments, and each segment is processed as soon as it arrives.

---

## 4. BPM Detection

**Algorithm:** Spectral flux onset detection → interval histogram → autocorrelation refinement

### Step 1: Onset Detection

```javascript
// Sliding-window spectral flux
// windowSize = (30 / maxTempo) * sampleRate
// For each sample position:
//   oldEnergy = sum of squares BEFORE window
//   newEnergy = sum of squares AFTER window  
//   flux = max(0, (1 - oldEnergy/newEnergy) * newEnergy) / (windowSize/2)
//   if flux > threshold → record as peak
//
// Threshold starts high, decrements by 5% of average until enough peaks found
```

The threshold adapts: starts at `averageEnergy - 5%`, decrements by `5%` each iteration until the peak count exceeds `(minTempo * trackLength)`. This ensures even quiet tracks find enough peaks.

### Step 2: Interval Histogram

```javascript
// For each pair of peaks within 10 positions of each other:
//   Compute interval in samples
//   Convert to BPM candidate
//   Score = peak count + weighted nearby tempos (±0.5 BPM)
// Sort by score descending
```

### Step 3: Enhanced Peak Detection (first 5 seconds)

The first 5 seconds get extra scrutiny with a separate, more aggressive detection pass. Additional peaks are merged into the main set.

### Step 4: Autocorrelation Refinement

```javascript
// For each candidate BPM, compute autocorrelation at 4, 8, 16 beat multiples
// Average the 3 values
// This tests whether the BPM makes musical sense at phrase level
```

### Step 5: Octave Error Resolution

```javascript
// Generate candidates at 1.5×, 0.67×, 2×, 0.5× the detected tempo
// Plus rounded versions
// Pick whichever has highest autocorrelation

// Genre-specific heuristics (hardcoded):
if (tempo ≈ 130.5)  → check 174 (DnB)
if (tempo 85-90)    → check ×2 (170-180)
if (tempo 90-115)   → double if correlation is higher
if (maxTempo > 220) → special handling for 135-145 and 180-195
```

### Step 6: Fine-tuning

```javascript
// Snap to integer if autocorrelation at integer ≥ 95% of float BPM
// Otherwise: search ±0.05 in 0.001 steps
// Check quarter/half BPM (x.25, x.5, x.75)
```

### Default Config

- **Tempo range:** 80-180 BPM (user configurable via `Analysis.tempoRange` setting)
- **Confidence:** ratio of top candidate score to total scores

---

## 5. Key Detection

**Algorithm:** Krumhansl-Schmuckler key-finding with custom chroma extraction

### Configuration

```javascript
{
    startOffset: 0.25,        // 25% into the audio slice
    sliceDuration: 0.5,       // Analyze 50% of the slice  
    lastFrequency: 1975.53,   // ~B6 (highest note)
    filterOrder: 110,         // FIR lowpass taps
    fftSize: 2048,
    frameSize: 16384,         // 2^14 samples per frame
    hopSize: 4096,            // 2^12 sample hop
    noteCount: 72,            // 6 octaves × 12 semitones
    octaveWeights: [0.400, 0.556, 0.525, 0.608, 0.599, 0.491],
    dskP: 0.9,                // Directional sparsity kernel bandwidth
    strengthNormalization: 0.8,
    marginWeighting: 15,
}
```

### Pipeline

```
Audio slice (30%-60% of track)
    │
    ├─ 1. FIR Lowpass: 110 taps, cutoff ≈ 1999 Hz
    │      Removes harmonics above musical range
    │
    ├─ 2. Downsample: 44100 → ~4400 Hz (10:1)
    │      Based on 2 × 1.1 × 1975 Hz
    │
    ├─ 3. Blackman Window: 16384-sample frames, 4096 hop
    │
    ├─ 4. Custom FFT: Hand-rolled Cooley-Tukey (2048-point)
    │
    ├─ 5. Triangular Kernel Binning:
    │      Map FFT bins → 72 pitch classes (6 octaves × 12 semitones)
    │      Bandwidth = 0.9 semitones (directional sparsity kernel)
    │      Normalize: divide by total energy per frame
    │
    ├─ 6. Average chroma: RMS-weighted mean across all frames
    │
    ├─ 7. Krumhansl-Schmuckler:
    │      For each of 24 keys (12 major + 12 minor):
    │        Pearson correlation between track chroma and rotated profile
    │
    └─ 8. Fallback chain:
           Best key → check silence → check major preference
           → check weak key → default "C"
```

### Key Profiles (Krumhansl-Kessler)

```
Major: [7.24, 3.50, 3.58, 2.85, 5.82, 4.56, 2.45, 6.99, 3.39, 4.56, 4.07, 4.46]
Minor: [7.00, 3.14, 4.36, 5.40, 3.67, 4.09, 3.91, 6.20, 3.63, 2.87, 5.35, 3.83]
```

Each profile is replicated across 6 octaves with per-octave weights, creating 72-element vectors. The 24 rotations (12 major + 12 minor) are tested via Pearson correlation.

### Confidence

```javascript
confidence = tanh(margin × normalized_strength)
// margin = gap between best and second-best key
// normalized_strength = min(best_score / 0.8, 1)
```

### External: OpenKeyScan

Optional key detection via external app (`localhost` HTTP). If enabled, the internal algorithm is skipped.

---

## 6. Beat Grid & First Beat

**This is the most complex part of the entire codebase (~1800 lines in module 358).**

### Strategy

1. Get BPM + peaks from `analyzeTempo()`
2. Convert peak positions from samples to seconds
3. Find the "first beat" — the first musically meaningful downbeat
4. Generate beat grid from there at detected BPM

### First Beat Detection — Multiple Fallbacks

```
Attempt 1: Check if first peak < 5ms
  ├─ If yes: Check if intro is silence (energy < 0.005)
  ├─ Check if peaks are evenly spaced (within 0.25-beat tolerance)
  └─ If evenly spaced → first peak IS the first beat

Attempt 2: Dynamic Onset
  ├─ findDynamicOnset(): finds sharpest energy rise near first peak
  └─ computeAttackRatio(): measures onset sharpness (post/pre RMS)

Attempt 3: Subsequent Beat Correction
  ├─ Walk forward from estimated first beat
  ├─ At each expected beat position, check for actual onset nearby
  ├─ Correct phase error based on accumulated drift
  └─ Returns validated first beat position

Attempt 4: Walk-Back
  ├─ From a known beat position later in the track
  ├─ Walk backward at BPM intervals
  ├─ At each step, check for strong onset
  └─ First strong onset from the left = first beat
```

### Genre-Special Handling

- **170-178 BPM**: Extra validation for DnB
- **200+ BPM**: Wider onset search window
- **85-90 BPM**: Check if it's actually half-time of 170-180

### Beat Grid Output

```javascript
tempomarkers = [{ startTime: 1.234, bpm: 128.00 }]
// Single entry: the first beat position + BPM
// All subsequent beats are computed: firstBeat + N × (60/bpm)
```

---

## 7. Cue Points & Section Detection

**Algorithm:** Energy-based phrase segmentation in bar-length (4-beat) segments

### Step 1: Per-Bar Analysis

```javascript
// For each bar (4 beats at BPM):
//   rmsEnergy = sqrt(mean(samples²)) over bar duration
//   beatStrength = average RMS at each beat position (±50ms window)
//   rampType = linear regression slope of beat strengths
//     "up" if slope > mean × threshold
//     "down" if slope < -mean × threshold  
//     "flat" otherwise
```

### Step 2: Segmentation

```javascript
// Split point = mean energy across all bars
// Group consecutive bars into high/low energy sections
// Minimum section length = breakdownMinBeats (default 64 beats = 16 bars)
// Round to 4-bar boundaries
```

### Step 3: Merge / Filter

```javascript
// Remove sections shorter than threshold
// If too many high-energy sections → remove shortest
// Remove high-energy sections in first 64 beats (intro region)
```

### Step 4: Assign Markers

```javascript
Start           → always at position 0
Drop            → first high-energy section
SecondDrop      → second high-energy section
Breakdown       → first low-energy section after drop
SecondBreakdown → second low-energy section
Lastbeat        → end of last high-energy section
```

### Step 5: Emergency Loop (S.O.S.)

```javascript
// Before the last drop, find a 16-beat section where:
//   Energy is within 90-110% of reference bar
//   AND per-beat energy pattern matches within 80-120%
// Relaxation: try strictest → energy-only → anything
// Marked as active loop with magenta color, named "S.O.S."
```

### Custom Anchor Points

Users can define custom cue point templates with specific marker types, colors, and cue types. These override the auto-generated section points.

---

## 8. Energy Rating

**Scale:** 1-10 (integer)  
**Input:** Lowpass-filtered mono signal, analyzed within drop regions

### Three Features

```
1. RMS Energy (50% weight)
   sqrt(mean(samples²)) within drop regions
   Falls back to 30-70% of track if no drops detected

2. Tempo Factor (30% weight)
   (bpm - 120) / 120
   Higher BPM → higher energy (electronic music bias)

3. Transient Density (50% weight)
   - Segment audio into 0.014-second windows
   - Compute RMS per segment
   - Count "strong beats": segments where RMS rise > 0.2 AND RMS > 0.3
   - density = (strong transients per second × 60)
   - Normalize: clamp((density - 550000) / 150000, 0, 1)
```

### Final Score

```javascript
score = 0.5 * rms + 0.3 * tempo_factor + 0.5 * density_factor

if (strongBeatCount <= 200) score *= 0.2   // heavy penalty for "flat" tracks

energy = clamp(round(9 * score) + 1, 1, 10)
```

The 0.2× penalty for tracks without strong beats ensures ambient/beatless tracks get low energy scores (1-3) even if they're loud.

---

## 9. Waveform Generation

### Worker 160: The Waveform Renderer

Uses a bundled DSP library (inlined from `dsp.js` / `fft.js`) with DFT, FFT, RFFT, biquad filters, window functions.

### Configuration

```javascript
{
    SAMPLE_RATE: 12000,         // Resampled from original
    FFT_SIZE: 128,              // Very small
    SEGMENT_WIDTH: 256,         // Samples per pixel column
    
    LOW:  [0, 150] Hz,         // Bass band
    MID:  [150, 1500] Hz,      // Mid band
    HIGH: [1500, 22050] Hz,    // Treble (Nyquist = 6kHz at 12kHz SR)
    
    LOW_WEIGHT: 1.2,           // Bass emphasized
    MID_WEIGHT: 1.0,
    HIGH_WEIGHT: 1.0,
    
    ALPHA: 0.7,                // Default transparency
    OVERVIEW_PADDING_SIDES: 5, // px
}
```

### Audio Loading Pipeline

```
1. ffmpeg splits audio into 5-second WAV segments
   ffmpeg -i input.mp3 -f segment -segment_time 5 output_%04d.wav

2. Each segment → OfflineAudioContext resample to 12kHz mono
   OfflineAudioContext(1, duration * 12000, 12000)
   → renders to mono Float32Array

3. Resampled data sent to Worker 160 as SharedArrayBuffer
   (shared memory between main thread and worker)
```

### Waveform Data Generation

```javascript
for each 256-sample window of 12kHz audio:
    // 1. Min/max amplitude for waveform shape
    min = minimum sample in window
    max = maximum sample in window
    
    // 2. FFT for color
    spectrum = FFT_128(forward)(window)
    low  = RMS(spectrum[low_bins])  * 1.2
    mid  = RMS(spectrum[mid_bins])  * 1.0
    high = RMS(spectrum[high_bins]) * 1.0
    
    // 3. Normalize to strongest band
    maxBand = max(low, mid, high)
    r = round(low  / maxBand * 255)  // bass = red channel
    g = round(mid  / maxBand * 255)  // mids = green channel
    b = round(high / maxBand * 255)  // treble = blue channel
    
    // 4. Smooth with previous segment
    color = blend(prev_color, computed_color, mix=0.5)
    
    // 5. Store
    segments.push({ min, max, color: { r, g, b, a: 0.7 } })
```

#### Rendering Notes (confirmed from Worker 160 source)
- **lineWidth = 2**, **lineCap = "round"** for all strokes
- **Zero-amplitude fallback**: `if (y_min === y_max)` → `strokeStyle = "rgb(80, 80, 80)"` (dark gray tick)
- **Coordinate formula**: `y = height - value * height + height/4`
  - Silence (0.0) → y = 1.25 × height (off-canvas bottom)
  - Full peak (1.0) → y = 0.25 × height (top quarter)
  - The waveform is **bottom-anchored**: peaks grow upward from the canvas floor
- **Overview is one-sided**: `moveTo(x, h/2); lineTo(x, h/2 - rmsHeight)` — asymmetric, upward only
- **Overview RMS**: `sqrt(mean(samples²)) × height × 2 × 0.9`

### Two Outputs

| Output | Format | Purpose |
|--------|--------|---------|
| **Overview** | WebP image (OffscreenCanvas → blob) | Full-track mini-view at top of player |
| **Zoom data** | Per-segment `{min, max, color}` arrays | Detailed scrollable waveform |

### Color Schemes via CSS Filters

The base 3-band color is the "blue-yellow" scheme. Other schemes use **CSS filters** on the `waveform-color-transformer` class:

| Scheme | CSS Filter |
|--------|-----------|
| `3band-blueyellow` | Native output |
| `3band-bluered` | Native variant |
| `rgb` | `hue-rotate(346deg)` |
| `blue-green` | `brightness(0.8) contrast(1.5) hue-rotate(115deg)` |
| `pink-blue` | `contrast(1.2) hue-rotate(218deg)` |
| `3band-custom` | User-configurable filter values |

This is clever — zero re-rendering needed to change color scheme. Just apply a CSS filter to the existing canvas/image.

---

## 10. Waveform UI: Scrolling View

### Component Hierarchy

```
BaseWaveform (abstract base class)
├── OverviewWaveform    — full-track mini view at top
└── ZoomWaveform        — detailed scrollable view below

WaveformRendererBase / WaveformRendererRgb
    — manages Worker 160, renders segments, caches bitmaps
```

### Canvas Architecture (3 layers per component)

```
┌─────────────────────────────────────────────┐
│  Overlay Canvas (beatgrid + cues + playhead) │  ← drawn every frame
├─────────────────────────────────────────────┤
│  Image Container (CSS translate3d)           │  ← GPU-positioned tiles
│  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐       │
│  │ Seg0 │ │ Seg1 │ │ Seg2 │ │ Seg3 │ ...    │  ← canvas elements
│  └──────┘ └──────┘ └──────┘ └──────┘       │
├─────────────────────────────────────────────┤
│  Container div (clips overflow)              │
└─────────────────────────────────────────────┘
```

### Overview Waveform

- **Source:** WebP blob loaded from `Waveform` table in database
- **Size:** 140px height (280px on high-DPI), full track width
- **Rendering:** `drawImage()` to canvas, overlay drawn on top
- **Interaction:** Click/drag → seek to position (maps pixel to 0-1 progress)

### Zoom Waveform

**Segment-based tile rendering:**

1. Audio is split into 5-second segments (matching `SEGMENT_TIME`)
2. Each segment gets its own `<canvas>` element
3. Worker 160 renders each segment via `drawZoom()`:
   - Creates `OffscreenCanvas`
   - Draws waveform lines (vertical strokes per pixel)
   - Returns `ImageBitmap` (zero-copy transfer to main thread)
   - Canvas uses `bitmaprenderer` context to display
4. Tiles positioned via CSS `transform: translate3D()`

**Zoom levels:**

```javascript
// 15 levels, each maps to a pixels-per-second multiplier
Step  1 → 1    (3000 px/s, most zoomed in)
Step  2 → 2    (1500 px/s)
Step  3 → 3    (1000 px/s)
Step  4 → 4    (750 px/s)
Step  5 → 5    (600 px/s)
Step  6 → 6    (500 px/s)
Step  7 → 8    (375 px/s)
Step  8 → 10   (300 px/s)
Step  9 → 12   (250 px/s)
Step 10 → 15   (200 px/s)
Step 11 → 20   (150 px/s)
Step 12 → 25   (120 px/s)
Step 13 → 30   (100 px/s)
Step 14 → 40   (75 px/s)
Step 15 → 50   (60 px/s, most zoomed out)

ZOOM_PX_PER_SEC = 3000  // at zoom scale 1 (most zoomed in)
// At scale 1, a 6-min track = 1,080,000 pixels
// At scale 50, a 6-min track = 21,600 pixels
```

**Tile positioning:**

```javascript
// Current pixel offset = progress × duration × ZOOM_PX_PER_SEC × zoomScale
// Each tile offset = centerOffset + cumulativeWidthOfPreviousTiles
tile.style.transform = `translate3D(${offset}px, 0, 0)`
```

This uses GPU-accelerated CSS transforms — no re-rendering, just matrix multiplication.

**Zoom waveform rendering modes (inside Worker 160):**

```
If samplesPerPixel <= 10 (very zoomed in):
    - For each pixel: aggregate nearby min/max values
    - Draw single vertical line per pixel (min to max)
    - Color = average of nearby segment colors
    - Apply continuity correction (prevent gaps)

If samplesPerPixel > 10 (zoomed out):
    - Sample the top-5 peaks and bottom-5 valleys
    - Weight peaks by rank (position 5× weight)
    - Draw single line from weighted average
    - Color = average across visible range
```

### Beat Grid Overlay

```javascript
drawBeatgrid() {
    for each tempomarker section:
        beatDuration = 60 / bpm / trackDuration  // normalized
        
        // Forward from first beat
        for (beat = 0; beat < totalBeats; beat++) {
            x = width / (zoomDistance / (beatTime - viewStart))
            
            if (beat % 4 === 0):           // bar line
                lineWidth = 1
                draw bar number label (Kufam font, 11px)
                if (beat % 16 === 0):      // phrase line  
                    bold text
                if (beat === 0):           // first beat
                    draw green rounded rect with BPM label at bottom
            else:                           // sub-beat line
                lineWidth = 0.5
                semi-transparent white/gray
        }
        
        // Also backward from first beat for pre-beat grid
}
```

### Cue Point Overlay

```javascript
for each cuepoint:
    x = width / (zoomDistance / (cue.startTime/duration - viewStart))
    
    if cue is loop:
        draw filled rectangle (width = loop duration)
        color = LOOP_BACKGROUND_COLOR / ACTIVE / PLAYING
    
    // Triangle markers at top and bottom
    draw upward triangle at (x, 0)    — tip points down
    draw downward triangle at (x, height) — tip points up
    
    // Dark theme: add shadow glow (shadowBlur = 5)
    color = cuepoint.color (28 presets) or white
```

### Playhead Indicator

```javascript
x = 0.5 * width - 1  // always at horizontal center

// Draw in two sections with gap for waveform visibility:
moveTo(x, 0)          → lineTo(x, 0.2 × height)    // top
moveTo(x, 0.8 × height) → lineTo(x, height)        // bottom

// Color: white (dark theme) / black (light theme)
// lineWidth = 2
```

### Interaction Model

| Input | Action |
|-------|--------|
| Mouse drag | Seek (throttled 10ms), shift = half speed |
| Double click | Seek to clicked position |
| Pinch gesture | Zoom in/out (debounced 100ms) |
| Touch pan | Seek |
| Mouse wheel | Not used for seeking (prevents accidents) |
| Keyboard shortcuts | Space = play/pause, arrows = seek ±beat |

### Track Browser Mini-Waveform

In the track list, each row shows a tiny waveform preview:

```html
<img class="waveform-preview waveform-color-transformer" 
     height="${rowHeight-2}px" 
     width="${maxWidth-24}px" 
     src="data:image/webp;base64,${track.waveform}" />
```

- The overview WebP is base64-encoded and embedded as a data URL
- The `waveform-color-transformer` class applies the current color scheme CSS filter
- A separate `previewCues` JSON array provides per-pixel cue indicators

---

## 11. Database & Storage

### Schema (Plain SQLite)

```sql
-- Core tables
Track              -- all metadata + analysis fields
Track_FTS          -- full-text search index
Playlist           -- tree structure (folders/playlists/smartlists)
Cuepoint           -- per-track cue/loop data
Tempomarker        -- per-track beat grid ({startTime, bpm})
Waveform           -- WebP overview BLOB + previewCues JSON
AlbumartPreview    -- album art thumbnails
Tag / TagCategory  -- custom tagging system
LinkTagTrack       -- track ↔ tag junction
LinkTrackPlaylist  -- track ↔ playlist ordered junction
Setting            -- app settings (key-value)
HistorySession / HistoryTrack  -- play history
CloudFile          -- cloud backup state
Database           -- DB metadata + UUID
```

### Waveform Table

```sql
CREATE TABLE "Waveform" (
    "trackId"     INTEGER NOT NULL UNIQUE,
    "data"        BLOB,              -- WebP overview image
    "previewCues" TEXT,              -- JSON array of cue indicators
    "iteration"   INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY("trackId") REFERENCES "Track"("id") ON DELETE CASCADE
);
```

### How Waveforms Are Saved

```javascript
// 1. Worker 160 renders overview to OffscreenCanvas
// 2. Export as WebP blob (quality 0.8)
const blob = await offscreenCanvas.convertToBlob({ type: "image/webp", quality: 0.8 })
const arrayBuffer = await blob.arrayBuffer()

// 3. Save to database
Buffer.from(arrayBuffer) → Waveform.data (BLOB)
```

### Preview Cues

JSON array stored alongside the waveform. Used for rendering tiny cue indicators on the track browser mini-waveform. Each entry maps a pixel position to a cue type.

---

## 12. Worker Architecture

### Worker Files

| File | Size | Role | Thread Library |
|------|------|------|---------------|
| `182.index.worker.js` | 624KB | Core analysis (BPM, key, energy, cues) | `threads.js` (comlink-style) |
| `160.index.worker.js` | 38KB | Waveform FFT + rendering | `threads.js` (comlink-style) |
| `940.index.worker.js` | 100KB | Utility (lodash + module code) | `threads.js` |
| `639.index.worker.js` | 155KB | Utility (lodash + helpers) | `threads.js` |
| `468.index.worker.js` | 8KB | Utility | `threads.js` |
| `110.index.worker.js` | 11KB | Utility | `threads.js` |

### Thread Communication

**Worker 182** (analysis) uses a **custom ThreadManager2**:

```javascript
class ThreadManager2 {
    constructor(concurrency, workerFactory) {
        // Pre-spawn N workers
        for (let i = 0; i < concurrency; i++) {
            const worker = workerFactory()  // new Worker("182.index.worker.js")
            workers.push({ worker, busy: false })
        }
    }
    
    addAsync(...args) {
        // Extract ArrayBuffers for zero-copy transfer
        const transferables = extractTransferables(args)
        // Queue task
        queue.push({ args, transferables, resolve, reject })
        schedule()
    }
    
    schedule() {
        // Find idle worker, dequeue task, postMessage with transferables
    }
}
```

**Worker 160** (waveform) uses **`threads.js`** library:

```javascript
// Exposes an object with methods:
expose({
    setup(width, height, padding, canvas),
    init(sampleRate, duration),
    appendData(segmentIndex, audioData, samplesPerSegment),
    drawZoom(canvas, segmentIndex, zoomLevel, height),
    drawOverviewSegment(segmentIndex, audioData),
    overviewToArraybuffer(),
    drawLoader(),
})
```

### Worker 182 Internal Message Format

```javascript
// Incoming (from main thread):
self.onmessage = function(event) {
    assert(event.data.length === 5)
    const [settings, sampleRate, buf_energyCues, buf_tempo, buf_key] = event.data
    
    const energyCuesAudio = new Float32Array(buf_energyCues)
    const tempoAudio = new Float32Array(buf_tempo)
    const keyAudio = new Float32Array(buf_key)
    
    // Process...
    
    self.postMessage(result)
}
```

### Worker 160 Internal Method Calls

```javascript
// Via threads.js RPC:
await thread.init(12000, duration)
await thread.appendData(segmentIndex, audioData, samplesPerSegment)
const bitmap = await thread.drawZoom(canvas, segmentIndex, zoomLevel, height)
await thread.drawOverviewSegment(segmentIndex, audioData)
const webpBuffer = await thread.overviewToArraybuffer()
```

---

## 13. Optimization Patterns

### Pattern 1: Aggressive Downsampling Before Processing

The single most impactful optimization. Every analysis step gets the minimum data it needs:
- Tempo: 200Hz effective (7 cascading lowpass stages)
- Key: 4400Hz after FIR + decimation
- Waveform: 12kHz (just above 6kHz Nyquist for 3-band separation)

### Pattern 2: OfflineAudioContext for Native Processing

Browser-native audio processing is orders of magnitude faster than JavaScript:
- `OfflineAudioContext.startRendering()` — native audio graph rendering
- `BiquadFilterNode` — native IIR filter implementation
- `AudioBuffer.getChannelData()` — typed array access

The entire preprocessing pipeline (decode + filter + resample) happens in native code.

### Pattern 3: Transferable ArrayBuffers

Data moves between threads with zero copies:
```javascript
// Main thread → Worker: ownership transfer, no memcpy
worker.postMessage(data, [data.buffer])
```

### Pattern 4: SharedArrayBuffer for Waveform

Waveform data uses `SharedArrayBuffer` — the main thread and worker can read the same memory simultaneously without transfers.

### Pattern 5: Incremental Segment Loading

Audio is split into 5-second segments by ffmpeg. Each segment is processed independently as it arrives. The UI can start rendering the waveform before the full track is loaded.

### Pattern 6: Canvas Tile Virtualization

Only visible waveform tiles are rendered. Off-screen tiles exist as canvas elements positioned via `translate3D` but aren't redrawn unless zoom level changes.

### Pattern 7: ImageBitmap Transfer for Zoom

Worker 160 renders zoom segments to `OffscreenCanvas`, then transfers `ImageBitmap` to main thread:
```javascript
// Worker side:
const bitmap = offscreenCanvas.transferToImageBitmap()
return bitmap  // transfers to main thread with zero copy

// Main thread side:
canvas.getContext("bitmaprenderer").transferFromImageBitmap(bitmap)
```

### Pattern 8: CSS Filter Color Schemes

Switching waveform color scheme is instant — just change a CSS `filter` property. No re-rendering, no worker communication.

### Pattern 9: Debounced Zoom Persistence

Zoom level is saved to settings with 1-second debounce, preventing excessive database writes during zoom gestures.

### Pattern 10: WebP for Overview Storage

WebP at quality 0.8 provides excellent compression for waveform images. A full-track overview might be 5-15KB vs 50-100KB for PNG.

---

## 14. What's Not Local

### Danceability / Happiness / Popularity

These fields (0-10 scale) are declared in the track model but **not computed by any local worker**. They're stored in the database and displayed in the UI, but the computation likely happens:

1. **Server-side** during Rekordcloud cloud sync
2. Or imported from external DJ software (Serato, Rekordbox)
3. Or set manually by the user

The `audio-analyzer` binary referenced in database migration code was likely an earlier attempt at local computation that was removed in v1.10.7.

### Cloud Analysis

Lexicon offers cloud-based analysis as part of its sync service. Tracks analyzed in the cloud would have these fields populated when synced back.

---

## Appendix A: Complete Audio Preprocessing Chains

### Tempo / Beatgrid / Cuepoints / Energy

```
File bytes
  → OfflineAudioContext.decodeAudioData()
  → AudioBuffer (44.1kHz stereo)
  → OfflineAudioContext (same sample rate)
  → AudioBufferSourceNode
  → BiquadFilterNode(frequency=800, type="lowpass")
  → BiquadFilterNode(frequency=400, type="lowpass")
  → BiquadFilterNode(frequency=400, type="lowpass")
  → BiquadFilterNode(frequency=200, type="lowpass")
  → BiquadFilterNode(frequency=200, type="lowpass")
  → BiquadFilterNode(frequency=200, type="lowpass")
  → BiquadFilterNode(frequency=200, type="lowpass")
  → destination
  → startRendering()
  → getChannelData(0) → Float32Array (mono, <200Hz content)
```

### Key Detection

```
File bytes
  → OfflineAudioContext.decodeAudioData()
  → AudioBuffer (44.1kHz stereo)
  → OfflineAudioContext(start=0.3×duration, duration=0.3×duration)
  → AudioBufferSourceNode → destination
  → startRendering()
  → getChannelData(0) → Float32Array (mono, raw, 30% slice)
  
  // Then inside Worker 182:
  → FIR lowpass (110 taps, 1999 Hz cutoff)
  → Decimate by ~10:1 → ~4400 Hz
  → Blackman window frames (16384 samples, 4096 hop)
  → FFT → chroma → Krumhansl-Schmuckler
```

### Waveform

```
File bytes
  → ffmpeg -f segment -segment_time 5 → 5s WAV files
  → OfflineAudioContext(1, duration×12000, 12000)
  → AudioBufferSourceNode → destination
  → startRendering()
  → getChannelData(0) → Float32Array (mono, 12kHz)
  → SharedArrayBuffer → Worker 160
  → 128-point FFT every 256 samples
  → 3-band RMS → RGB color
  → OffscreenCanvas → WebP (overview)
  → {min, max, color}[] (zoom data)
```

## Appendix B: Beat Grid Numbering System

Lexicon supports two numbering modes:

| Mode | Description |
|------|------------|
| `"0"` (Zero) | First bar = 0 |
| `"1"` (FirstMarker) | First bar = 1 (the tempomarker) |

The `getBarNumberAtTime()` function converts a time position to a bar number using the tempomarkers array. Beat lines show bar numbers at every 4th beat (every bar). Every 16th beat (every phrase) gets bold text.

## Appendix C: Key Constants Reference

```
SAMPLE_RATE (waveform)        = 12000 Hz
waveformSamplerate (analysis) = 3000 Hz
SEGMENT_TIME                  = 5 seconds (desktop), 30 seconds (mobile)
ZOOM_PX_PER_SEC               = 3000 pixels/second (at zoom 1×)
MIN_ZOOM                      = 1
MAX_ZOOM                      = 15
OVERVIEW_HEIGHT               = 140px (280px high-DPI)
OVERVIEW_PADDING_SIDES        = 5px
FFT_SIZE (waveform)           = 128 samples
SEGMENT_WIDTH (waveform)      = 256 samples/pixel
LOW_FREQ_RANGE                = 0-150 Hz
MID_FREQ_RANGE                = 150-1500 Hz
HIGH_FREQ_RANGE               = 1500-22050 Hz
LOW_WEIGHT                    = 1.2
MID_WEIGHT                    = 1.0
HIGH_WEIGHT                   = 1.0
DEFAULT_TEMPO_MIN             = 80 BPM
DEFAULT_TEMPO_MAX             = 180 BPM
STRONG_BEAT_RMS_THRESHOLD     = 0.3
STRONG_BEAT_RISE_THRESHOLD    = 0.2
MIN_STRONG_BEATS              = 200
DEFAULT_BREAKDOWN_MIN_BEATS   = 64
```
