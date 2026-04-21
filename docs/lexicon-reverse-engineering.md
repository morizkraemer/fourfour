# Lexicon DJ v1.10.7 — Complete Reverse Engineering

**Bundle ID:** `com.rekord.cloud.lexicon`  
**Author:** Rekordcloud (Christiaan Maks)  
**Framework:** Electron (Atom-based), with Python tagger subprocess  
**Platform:** macOS arm64 / Windows  

---

## 1. Application Architecture

### Runtime & Stack
- **Electron app** (JavaScript/Node.js) with WebUI via Webix framework
- **macOS data:** `~/Library/Application Support/Lexicon/`
- **Windows data:** `%APPDATA%/Lexicon/`
- **Local HTTP API:** `http://localhost:48624/v1/` (opt-in via settings)
- **JS Framework:** Webix UI (datatable, tree, layout components) + Vue.js for music player controls
- **Worker library:** `threads.js` (comlink-style worker pool with transferable ArrayBuffers)

### Database
- **SQLite** file: `main.db` — **no encryption** (plain SQLite, unlike Rekordbox's SQLCipher)
- WAL mode used
- Libraries can be swapped by replacing this file/symlink (Multi-DB Manager tool does this)

### Database Tables (from code)
```
Database, Track, Track_FTS, 
View_TracksNonArchived, View_TracksArchived, View_TracksIncoming,
Playlist, Setting, ChartItem,
TagCategory, Tag, Cuepoint, Tempomarker,
AlbumartPreview, Waveform, LinkTagTrack, LinkTrackPlaylist,
CloudFile, HistorySession, HistoryTrack
```

### Bundled Tools
| Tool | Path | Purpose |
|------|------|---------|
| ffmpeg | `lib/ffmpeg/ffmpeg` | Audio decoding (format conversion) |
| ffprobe | `lib/ffmpeg/ffprobe` | Audio file metadata probing |
| lame 3.100 | `lib/lame/lame` | MP3 encoding (re-encoding during sync) |
| lexicon-tagger | `lib/lexicon-tagger/` | Python 3.12 PyInstaller binary for ID3 tag R/W via eyed3 |
| better-sqlite3 | `node_modules/` | SQLite database driver |

---

## 2. Plugin System

JavaScript sandbox with scoped API:

| Global | Purpose |
|--------|---------|
| `_vars.tracksSelected` | Array of selected track objects |
| `_settings` | User-configurable plugin settings |
| `_helpers.Log(msg)` | Logging |
| `_ui.showInputDialog(options)` | Interactive dialogs |
| `_network.GET({url, headers})` | HTTP GET (permission-gated) |
| `_library.track.getNextAllBatch()` | Async batch iteration over all tracks |

Permissions: `track.read`, `track.modify`, `track.modifyFields`, `network.GET`

---

## 3. Track Data Model (Full Schema from Code)

### Editable Fields (via API & UI)
| Field | Type | Bounds | Notes |
|-------|------|--------|-------|
| `title`, `artist`, `albumTitle` | string | — | id3Writable |
| `label`, `remixer`, `mix`, `composer` | string | — | id3Writable |
| `producer`, `grouping`, `lyricist`, `comment` | string | — | id3Writable |
| `key` | string | — | id3Writable, DJ notation ("4A") |
| `genre` | string | — | id3Writable, has auto-fill |
| `rating` | number | 0-5 | id3Writable |
| `color` | enum | 28 presets | — |
| `year` | number | — | id3Writable |
| `trackNumber` | number | — | id3Writable |
| `energy` | number | **0-10** | Lexicon analysis |
| `danceability` | number | **0-10** | Server-side analysis |
| `popularity` | number | **0-10** | Server-side analysis |
| `happiness` | number | **0-10** | Server-side analysis |
| `extra1`, `extra2` | string | — | Custom fields |

### Read-Only Fields
| Field | Type | Notes |
|-------|------|-------|
| `bpm` | number | **Read-only via API!** Set only by internal analysis |
| `duration` | number | seconds, max 65536 |
| `bitrate` | number | kbps |
| `sampleRate` | number | Hz |
| `fileType` | enum | mp3/mp4/m4a/aiff/wav/ogg/flac/alac/aac |
| `sizeBytes` | number | — |
| `fingerprint` | string | Chromaprint for duplicate detection |
| `beatshiftCase` | string (A-D) | Beat-shift classification |

### Internal Fields
| Field | Type | Notes |
|-------|------|-------|
| `waveform` | string | Base64 WebP blob (overview image) |
| `previewCues` | array | JSON: per-pixel cue indicators for track browser |
| `tempomarkers` | array | Beat grid (`{startTime, bpm}` pairs) |
| `cuepoints` | array | Full cue/loop data |
| `incoming` | boolean | Whether track is in incoming queue |
| `archived` | boolean | Soft-delete flag |
| `importSource` | enum | -4 to 11 (Serato4..PioneerUsb) |
| `data` | object | Per-format metadata (e.g. `data.rekordbox.discNumber`) |

### Cuepoint Object
- `type`: 1=normal, 2=fade-in, 3=fade-out, 4=load, 5=loop
- `position`: integer index (unique per track)
- `activeLoop`: boolean
- `color`: 28 preset colors
- `data.rekordbox.originalNum`: Rekordbox memory cue marker

---

## 4. Audio Analysis Pipeline — WHY IT'S FAST

### Key Insight: 12kHz Downsampled Audio

The single biggest reason Lexicon is fast:

> **Waveform analysis uses a 12kHz sample rate** (`SAMPLE_RATE = 12e3`)
> **Tempo/energy/cuepoint analysis uses 200Hz lowpass-filtered audio**
> **Key detection uses a 1975Hz lowpass-filtered, downsampled slice**

A 6-minute stereo 44.1kHz track has ~31.7M samples. Lexicon reduces this to:
- **Waveform**: ~4.3M samples (12kHz mono) → 5× less data
- **Tempo/energy/cues**: ~720K effective samples (200Hz mono) → 44× less data
- **Key**: ~79K samples (1975Hz downsampled, 30% slice) → 400× less data

Compare to Rekordbox which likely processes at full sample rate.

### Worker Pool Architecture

```
AudioAnalyzer {
  concurrency = clamp(trackCount)  // spawns N worker threads
  workerFactory = () => new Worker("182.index.worker.js")
  threadManager = ThreadManager2(concurrency, workerFactory)
}
```

`ThreadManager2` is a **custom worker pool** (not the `threads.js` pool):
- Pre-spawns `concurrency` workers (one per track, up to a limit)
- Each worker gets **5 transferable ArrayBuffers** (zero-copy)
- Workers process independently, no shared state

### Audio Decoding Pipeline

All audio decoded using **Web Audio API** (`OfflineAudioContext`), not ffmpeg:

```
1. TrackDiskLoader reads raw file bytes
2. OfflineAudioContext.decodeAudioData() → AudioBuffer (full sample rate)
3. THREE separate preprocessing paths run in parallel:
```

#### Path 1: Tempo / Beatgrid / Cuepoints / Energy
```javascript
// FULL-TRACK, 7-stage cascading lowpass → extreme bass isolation
OfflineAudioContext → 
  BiquadFilterNode(800Hz, lowpass) →
  BiquadFilterNode(400Hz, lowpass) →
  BiquadFilterNode(400Hz, lowpass) →
  BiquadFilterNode(200Hz, lowpass) →
  BiquadFilterNode(200Hz, lowpass) →
  BiquadFilterNode(200Hz, lowpass) →
  BiquadFilterNode(200Hz, lowpass) →
  destination
// Result: mono Float32Array, effectively <200Hz content only
```
**Why 7 cascading filters?** Each `BiquadFilterNode` has a -12dB/oct slope. Seven stages = -84dB/oct, creating an extremely steep anti-aliasing + bass isolation filter. This removes everything above ~200Hz, leaving only kick drums and bass. The `OfflineAudioContext` renders this in a single pass — much faster than processing full-spectrum audio.

#### Path 2: Key Detection
```javascript
// 30%–60% of track duration, UNFILTERED
OfflineAudioContext(start=0.3*duration, duration=0.3*duration)
// No filter chain — raw audio, mono channel
// Result: mono Float32Array, ~30% of track
```

#### Path 3: Waveform
```javascript
// 12kHz resampled mono audio via AudioContext
SAMPLE_RATE = 12000
// Segmented into SEGMENT_TIME (e.g. 30s) chunks
// Each segment → Worker 160 for FFT color rendering
```

### What Gets Sent to Worker 182

```javascript
// 5 transferable ArrayBuffers (zero-copy to worker thread):
t.data[0] = settings  // {analyzeBeatgrid, minTempo, maxTempo, analyzeCuepoints, ...}
t.data[1] = sampleRate // number
t.data[2] = Float32Array  // lowpass-filtered mono → for cuepoints & energy
t.data[3] = Float32Array  // lowpass-filtered mono → for tempo/beatgrid
t.data[4] = Float32Array  // unfiltered mono → for key detection
```

**Note:** [2] and [3] appear to be the same filtered data, but [3] is also used with the full unfiltered buffer for beat grid computation.

### Worker 182: Internal Module Map

| Module | Export | Lines | Purpose |
|--------|--------|-------|---------|
| 745 | `analyzeTempo` | ~250 | BPM detection (onset + autocorrelation) |
| 358 | `Wi` | ~1800 | Beatgrid / first-beat / drop detection |
| 40 | `DEFAULT_DROP_SETTINGS` | ~30 | Default config for cue point generation |
| 578 | correction helpers | — | Beat position correction |
| 974 | debug helpers | — | Debug field stripping |
| 473 | beat correction | — | Subsequent beat correction |
| 382 | walk-back | — | Walk-back first beat finder |
| 522 | debug logging | — | Debug flag management |

Plus: Section point detection (function `M`), energy detection (function `A`), key detection (function `O`) — all inline in the main module.

---

## 5. Analysis Algorithms — Complete Code-Level Details

### 5.1 BPM / Tempo Detection (`analyzeTempo`, module 745)

**Algorithm: Spectral flux onset detection → interval histogram → autocorrelation refinement**

#### Step 1 — Onset Detection (function `n`)
```javascript
// Sliding-window spectral flux
// For each sample position:
//   o = sum of squares BEFORE window (old energy)  
//   i = sum of squares AFTER window (new energy)
//   flux = max(0, (1 - o/i) * i) / (windowSize/2)
//   if flux > threshold → record peak
//   Window size = (30/maxTempo) * sampleRate
// Threshold starts high, decrements by 5% of average until enough peaks found
```

#### Step 2 — Peak Interval Histogram (function `o`)
```javascript
// For each pair of peaks within 10 of each other:
//   Compute interval in samples
//   Convert to BPM candidate
//   Score = peak count + weighted nearby tempos (±0.5 BPM, weighted by distance)
// Sort by score descending
```

#### Step 3 — Enhanced Peak Detection (function `i`)
```javascript
// First 5 seconds get extra scrutiny
// Uses separate threshold loop with narrower windows
// Additional peaks added to complement the main set
// Combined peak set = union of both methods
```

#### Step 4 — Autocorrelation Refinement (functions `a`, `u`)
```javascript
// For each candidate BPM:
//   Compute autocorrelation at 4, 8, 16 beat multiples
//   Average the 3 correlation values
// 
// Autocorrelation (function a): standard normalized cross-correlation
//   r = sum(|x[i] * x[i-offset]|) / sqrt(sum(x[i]²) * sum(x[i-offset]²))
```

#### Step 5 — Octave Error Resolution
```javascript
// Test at 1.5x, 0.67x, 2x, 0.5x the detected tempo
// Plus rounded versions of each
// Pick whichever has highest autocorrelation

// Genre-specific heuristics:
if (tempo ≈ 130.5) → might be 174 (DnB)
if (tempo 85-90)   → might double to 170-180
if (tempo 90-115)  → doubles if correlation is higher
if (maxTempo > 220 && tempo 135-145) → check double
if (maxTempo > 220 && tempo 180-195) → check 1.5x
```

#### Step 6 — Fine-tuning
```javascript
// Snap to integer if autocorrelation ≥ 95% of float BPM
// Otherwise search ±0.05 BPM in 0.001 steps
// Check for quarter/half BPM (x.25, x.5, x.75)
```

### 5.2 First Beat / Beat Grid Detection (`Wi`, module 358)

**This is the most complex part of the entire analysis (~1800 lines)**

#### Overview
1. Call `analyzeTempo()` to get BPM + peaks
2. If peaks exist and first peak < 5ms → check if it's silence
3. Call `findDynamicOnset()` for each candidate position
4. `computeAttackRatio()` — measures onset sharpness
5. `correctFirstBeatFromSubsequentBeats()` — phase error correction
6. `walkBackToFirstBeat()` — backward beat-grid construction
7. Multiple validation passes with genre-specific heuristics

#### Key Sub-functions
- **`computeAttackRatio(samples, sr, pos, windowMs)`** — Ratio of post-onset RMS to pre-onset RMS. Higher = sharper attack.
- **`findDynamicOnset(samples, sr, pos, windowMs, threshold)`** — Refines an approximate onset position by finding the sharpest energy rise nearby
- **`correctFirstBeatFromSubsequentBeats()`** — Walks forward from first beat, checks expected vs actual onset positions, corrects phase error
- **`walkBackToFirstBeat()`** — From a known beat position, walks backward at BPM intervals looking for strong onsets

### 5.3 Key Detection (`O` function — Krumhansl-Schmuckler)

**Complete configuration (from code):**
```javascript
{
    startOffset: 0.25,           // Start 25% into the analysis slice
    sliceDuration: 0.5,          // Analyze 50% of the slice
    decodingSampleRate: 44100,   
    lastFrequency: 1975.53,      // ~B6, highest note
    lpfCutoffMultiplier: 1.012,  // LPF slightly above lastFrequency
    dsCutoffMultiplier: 1.1,     // Downsample cutoff
    filterOrder: 110,            // FIR filter taps
    fftSize: 2048,               // FFT size
    hammingAlpha: 0.54,          // Hamming window
    hammingBeta: 0.46,
    frameSizePower: 14,          // 2^14 = 16384 sample frames
    hopSizePower: 12,            // 2^12 = 4096 sample hop  
    noteCount: 72,               // 6 octaves × 12 semitones
    octaveCount: 6,
    a440Frequency: 440,
    dskP: 0.9,                   // Directional sparsity kernel bandwidth
    silenceSensitivity: 0.05,
    strengthNormalization: 0.8,
    marginWeighting: 15,
    majorPreferenceThreshold: 0.02,
    weakKeyThreshold: 0.2,
    minimumKeyScore: 0.1,
    octaveWeights: [
        0.3999726755, 0.5563442524830065, 0.5249663634514354,
        0.6084754838427773, 0.5989811568, 0.49072435317960994
    ],
    kernelWindowType: "hann",
    defaultKey: "C",
}
```

#### Processing Steps
1. **Slice**: Take 25%-75% of the audio slice
2. **FIR Lowpass**: 110-tap filter, cutoff = 1975.53 × 1.012 ≈ 1999 Hz
3. **Downsample**: By `sr / (2 × 1.1 × 1975)` ≈ 10:1 (44100 → ~4400 Hz)
4. **Blackman Window Frames**: 16384-sample frames, 4096-sample hop
5. **Custom FFT**: Hand-rolled Cooley-Tukey FFT (no library!)
6. **Triangular Kernel Binning**: Map FFT bins → 72 pitch classes with directional sparsity kernel (bandwidth 0.9 semitones)
7. **Normalize per frame**: Divide by total energy → chroma vector
8. **Average across frames**: RMS-weighted mean chroma
9. **Krumhansl-Schmuckler**: Pearson correlation against 24 rotated key profiles

**Major profile:** `[7.24, 3.50, 3.58, 2.85, 5.82, 4.56, 2.45, 6.99, 3.39, 4.56, 4.07, 4.46]`  
**Minor profile:** `[7.00, 3.14, 4.36, 5.40, 3.67, 4.09, 3.91, 6.20, 3.63, 2.87, 5.35, 3.83]`

Each profile is replicated across 6 octaves with per-octave weights.

10. **Confidence**: `tanh(margin × normalized_strength)` where margin = gap between best and second-best key
11. **Silence detection**: If best score ≤ zero-vector correlation → "SILENCE"
12. **Fallback chain**: Major preference → weak key → first key → default "C"

**External option:** OpenKeyScan via HTTP localhost for key detection.

### 5.4 Cue Point / Section Detection (`M` function)

**Algorithm: Energy-based phrase segmentation in bar-length (4-beat) segments**

#### Step 1 — Per-bar Analysis
```javascript
// For each bar (4 beats at BPM):
//   RMS energy of the bar
//   Beat strength = average RMS at each beat position (±50ms window)
//   Ramp type = linear slope of beat strengths (up/down/flat)
//     Computed via linear regression on 4 beat positions
//     Threshold = mean beat strength × 0.05
```

#### Step 2 — Segmentation
```javascript
// Iterate bars, group into high/low energy sections:
//   Split point = mean energy across all bars
//   Each section must be ≥ minBeats (default 64) long
//   Round to 4-bar boundaries
//   Low-energy sections: look ahead for beat strength recovery
```

#### Step 3 — Merge / Filter
```javascript
// Remove sections shorter than threshold
// If too many high-energy sections (> maxDrops):
//   Remove shortest low-energy section and its neighbor
//   Repeat until count is correct
// Remove high-energy sections in first 64 beats (intro)
```

#### Step 4 — Assign Markers
```javascript
// Start → always at beginning
// Drop → first high-energy section
// SecondDrop → second high-energy
// Breakdown → first low-energy after drop  
// SecondBreakdown → second low-energy
// Lastbeat → end of last high-energy section
```

#### Step 5 — Emergency Loop (S.O.S.)
```javascript
// Find 16-beat section before last drop where:
//   Energy is within 90-110% of reference bar
//   AND beat pattern is within 80-120% of reference per-beat
// Try strictest match first, relax constraints on failure
```

### 5.5 Energy Rating (0-10 scale, function `A`)

```javascript
// Operates on the lowpass-filtered mono signal, within drop regions
// Falls back to 30-70% of track if no drops found

// Three features:
// 1. RMS energy (50% weight): sqrt(mean(samples²)) in drop regions
// 2. Tempo factor (30% weight): (bpm - 120) / 120
// 3. Transient density (50% weight):
//    - Compute per-segment RMS in windows of 0.014 seconds
//    - Detect "strong beats": segments where RMS rise > 0.2 AND RMS > 0.3
//    - Count strong beats > 200 → flag as "has any strong beats"
//    - Density = (strong transients per second) × 60, then normalize

// Score computation:
score = 0.5 * rms + 0.3 * tempo_factor + 0.5 * density_factor
if (!hasAnyStrongBeats) score *= 0.2  // heavy penalty
return clamp(round(9 * score) + 1, 1, 10)
```

**Key constants:**
- Strong beat RMS threshold: 0.3
- Strong beat rise threshold: 0.2
- Minimum strong beats to count: 200 (over ~5 minutes = ~0.67/sec)
- Transient detection window: 0.014 seconds
- Normalization constant for density: 550,000 (15,000 range)

### 5.6 Danceability / Happiness / Popularity

These fields are **declared in the track model** (0-10 bounds, `skipMerge: true`) but **NOT computed by any local worker**. The analysis pipeline only produces BPM, key, energy, cue points, and waveform data.

**Conclusion:** These are computed by the **Rekordcloud server** during cloud sync analysis, or set manually by the user. The `audio-analyzer` binary referenced in database migration code may have been an earlier attempt that was removed in this version.

---

## 6. Waveform Generation — Complete Pipeline

### Worker 160 Architecture

The waveform worker uses a **custom FFT library** (not a DSP npm package — the entire FFT implementation is inlined from `fft.js` / `dsp.js` libraries):

- **DFT**: Standard O(N²) DFT for small buffers
- **FFT**: Radix-2 Cooley-Tukey for power-of-2 sizes  
- **RFFT**: Real-valued FFT with bit-reversal permutation optimization
- Additional DSP: Biquad filters, IIR filters, window functions (Bartlett, Blackman, Hamming, Hann, etc.)

### Waveform Configuration

```javascript
{
    SAMPLE_RATE: 12000,           // 12kHz! Key speed optimization
    FFT_SIZE: 128,                // Very small FFT
    SEGMENT_WIDTH: 256,           // Samples per pixel column
    ALPHA: 0.7,                   // Default color alpha
    
    // Frequency bands
    LOW:  [0, 150] Hz,           // Bass
    MID:  [150, 1500] Hz,        // Mids
    HIGH: [1500, 22050] Hz,      // Highs (but at 12kHz SR, Nyquist = 6kHz)
    
    // Band weights  
    LOW_WEIGHT: 1.2,             // Bass emphasized
    MID_WEIGHT: 1.0,
    HIGH_WEIGHT: 1.0,
}
```

### Waveform Generation Steps

#### 1. Data Append (`appendData`)
```javascript
// Receives Float32Array of 12kHz mono audio
// For each segment of 256 samples:
//   Compute min/max amplitude
//   Run 128-point FFT → magnitude spectrum
//   Compute RMS energy in 3 frequency bands
//   Normalize to strongest band → RGB color (0-255 per channel)
//   Store {min, max, color: {r, g, b, a}}
```

#### 2. Color Computation
```javascript
// For each 256-sample window:
spectrum = FFT(forward)(window)
low  = RMS(spectrum[0..14])   * 1.2   // 0-150Hz at 12kHz/128 ≈ 93.75 Hz/bin
mid  = RMS(spectrum[15..143])  * 1.0  // 150-1500Hz  
high = RMS(spectrum[144..511]) * 1.0  // 1500-6000Hz

// Normalize relative to strongest band:
maxBand = max(low, mid, high)
r = round(low  / maxBand * 255)
g = round(mid  / maxBand * 255)  
b = round(high / maxBand * 255)
```

#### 3. Color Smoothing
```javascript
// Adjacent segments are blended for visual smoothness:
actual_color = blend(previous_color, raw_color, 0.5)
```

### Waveform Color Schemes

The 3-band RGB is the **native** color. Additional schemes use **CSS filters** on the `waveform-color-transformer` class:

| Scheme | CSS Filter |
|--------|-----------|
| `3band-blueyellow` | Native (low=blue-ish, mid=green, high=bright) |
| `3band-bluered` | Native variant |
| `3band-custom` | User-configurable |
| `rgb` | `hue-rotate(346deg)` |
| `blue-green` | `brightness(0.8) contrast(1.5) hue-rotate(115deg)` |
| `pink-blue` | `contrast(1.2) hue-rotate(218deg)` |

### Waveform Storage

**Overview waveform:**
- Rendered to OffscreenCanvas by Worker 160
- Saved as **WebP blob** (quality 0.8)
- Stored in `Waveform` table as BLOB
- Also stored as **base64** on the Track object for quick UI loading

**Preview cues:**
- JSON array of per-pixel cue indicators
- Stored in `Waveform.previewCues` column (TEXT)
- Used for mini-waveform display in track browser rows

**Zoom waveform:**
- Rendered on-demand by Worker 160 (`drawZoom`)
- Uses stored `{min, max, color}` data
- Supports 15 zoom levels (1× to 50× pixels-per-second)
- Canvas tiles positioned via `translate3D` CSS transforms

**Database schema:**
```sql
CREATE TABLE "Waveform" (
    "trackId"   INTEGER NOT NULL UNIQUE,
    "data"      BLOB,              -- overview WebP image
    "previewCues" TEXT,            -- JSON array
    "iteration" INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY("trackId") REFERENCES "Track"("id") ON DELETE CASCADE
);
```

### 6.1 — Source-Confirmed Rendering Details (Worker 160)

**Confirmed from actual `160.index.worker.js` source code:**

1. **Fallback color for silence**: When a window has zero amplitude (y_min === y_max), Lexicon uses `"rgb(80, 80, 80)"` dark gray instead of transparent. Zero-height lines still render as a gray tick, ensuring visibility even in silence regions.

2. **Coordinate formula (zoom waveform)**: Y-positions use `y = height - value * height + height/4`. The baseline (silence = 0.0) maps to `y = height * 1.25` (off-canvas bottom). The waveform is **bottom-anchored** — peaks grow upward from the canvas floor, not symmetric from center.

3. **Line styling**: `lineWidth = 2` for all waveform strokes; `lineCap = "round"` for smooth endpoints.

4. **Zero-height guard**: `if (y_min === y_max) { use fallback color } else { use rgba(r,g,b,0.7) }`. This prevents invisible lines in silence.

5. **Overview is one-sided RMS**: `drawOverviewSegment` draws `moveTo(x, height/2); lineTo(x, height/2 - rms_height)` — upward from center only, not symmetric. RMS formula: `sqrt(mean(samples²)) * height * 2 * 0.9`.

6. **Color RMS (confirmed true RMS)**: `sqrt(mean(magnitude_bins²))` over each band's FFT bins — this is the true RMS of spectral magnitudes, not a sum or average of magnitudes.

---

## 7. Waveform UI Rendering — How It Becomes a Scrolling View

### Component Hierarchy

```
BaseWaveform (abstract)
├── OverviewWaveform   — full-track mini-view at top
└── ZoomWaveform       — scrollable detailed view
```

### Canvas Architecture (3-layer)

Each waveform component uses **3 stacked canvases**:

1. **Waveform Canvas** — the actual waveform image (either overview WebP or zoom tiles)
2. **Overlay Canvas** — beat grid lines, cue point markers, playhead indicator
3. **Image Container** — CSS-transformed div holding waveform tiles

### Overview Waveform

```html
<div class="musicplayer-overview">
  <canvas class="waveform-canvas" />       <!-- Waveform image -->
  <canvas class="overlay-canvas" />         <!-- Cuepoints + playhead -->
</div>
```

**Rendering:**
- Loaded from stored WebP blob via `loadCachedFullOverviewWaveform()`
- Or rendered segment-by-segment by Worker 160 (`drawOverviewSegment`)
- Cue points drawn as colored triangles at top + bottom
- Active loops drawn as semi-transparent rectangles
- Playhead drawn as thin vertical line

**Interaction:**
- Click/drag → seek to position (maps pixel offset to 0-1 progress)
- Uses `OVERVIEW_PADDING_SIDES = 5px`

### Zoom Waveform

```html
<div class="musicplayer-tiles-container">
  <div class="musicplayer-tiles">           <!-- CSS translate3d container -->
    <canvas data-segmentindex="0" />         <!-- Segment 0 waveform tile -->
    <canvas data-segmentindex="1" />         <!-- Segment 1 waveform tile -->
    ...
  </div>
  <canvas class="overlay-canvas" />          <!-- Beatgrid + cues + playhead -->
</div>
```

**Segment-based rendering:**
1. Audio is split into `SEGMENT_TIME`-second chunks (e.g., 30s)
2. Each chunk becomes a `<canvas>` element
3. Worker 160 renders each segment via `drawZoom()`:
   - Uses `OffscreenCanvas` → renders to `ImageBitmap`
   - Transfers bitmap to main thread (zero-copy)
4. Tiles are positioned via CSS `translate3D` for GPU-accelerated scrolling

**Zoom levels:**
```javascript
getZoomLevel() {
    // 15 levels: 1× to 50× pixels-per-second
    step 1 → 1, step 2 → 2, ..., step 7 → 8, step 8 → 10,
    step 9 → 12, step 10 → 15, step 11 → 20, step 12 → 25,
    step 13 → 30, step 14 → 40, step 15 → 50
}
ZOOM_PX_PER_SEC = 3000  // pixels per second at base zoom
// At zoom 1×: 3000 px/s → a 6-min track = 1,080,000 pixels
// At zoom 50×: 60 px/s → a 6-min track = 21,600 pixels
```

Wait — that's inverted. Let me re-read:
```javascript
getZoomScale(level) = 1 / level
// Zoom level 1 → scale 1 → 3000 px/s (most zoomed in)
// Zoom level 15 → scale 1/50 → 60 px/s (zoomed out)
```

**Repositioning tiles:**
```javascript
repositionWaveform() {
    // Calculate pixel offset for current play position
    let offset = progress * duration * ZOOM_PX_PER_SEC * getZoomScale()
    
    // For each tile:
    tile.style.transform = `translate3D(${centerOffset + cumulativeWidth}px, 0, 0)`
}
```

### Beat Grid Overlay

```javascript
drawBeatgrid() {
    // For each tempomarker (beat grid section):
    const beatDuration = 60 / bpm / trackDuration  // normalized
    
    for (let beat = 0; beat < totalBeats; beat++) {
        const x = width / (zoomDistance / (beatTime - viewStart)) - lineWidth/2
        
        if (beat % 4 === 0) {
            // Bar line — thick, with bar number label
            drawBeatLine(beat, ...)  // draws vertical line + number
            
            if (beat % 16 === 0) {
                // Phrase line — bold, white text
            }
            
            if (beat === 0) {
                // First beat — colored rounded rectangle with BPM label
            }
        } else {
            // Sub-beat line — thin, semi-transparent
        }
    }
    
    // Also draw backward from first marker for pre-beat beats
}
```

**Beat line drawing details:**
- Bar lines: 1px wide, with bar number in Kufam font (11px scaled)
- Phrase lines (every 16th beat): bold text
- First beat: green rounded rectangle with BPM text at bottom
- Sub-beat lines: 0.5px wide, 80% opacity white (dark theme) / gray (light theme)
- Bar numbers: right-aligned, clear background rect behind text

### Cue Point Overlay

```javascript
drawCuepoints() {
    for (const cue of cuepoints) {
        const x = width / (zoomDistance / (cue.startTime/duration - viewStart))
        
        if (cue.type === Loop) {
            // Draw loop region as filled rectangle
            // Color depends on state: normal / active / playing
            ctx.fillRect(x, 0, loopWidth, height)
        }
        
        // Draw cue marker as two triangles (top + bottom)
        ctx.moveTo(x, 10)
        ctx.lineTo(x + 5, 0)
        ctx.lineTo(x - 5, 0)  // upward triangle at top
        
        ctx.moveTo(x, height - 10)
        ctx.lineTo(x + 5, height)
        ctx.lineTo(x - 5, height)  // downward triangle at bottom
        
        // Dark theme: add glow (shadowBlur = 5)
    }
}
```

### Playhead Indicator

```javascript
drawIndicator() {
    const x = 0.5 * width - 1  // Always at center
    
    // Top section (0-20% height)
    ctx.moveTo(x, 0)
    ctx.lineTo(x, 0.2 * height)
    
    // Bottom section (80-100% height)  
    ctx.moveTo(x, 0.8 * height)
    ctx.lineTo(x, height)
    
    // Gap in middle for waveform visibility
}
```

### Interaction

- **Mouse drag**: Throttled seek (10ms), shift = half speed
- **Double click**: Seek to clicked position
- **Pinch**: Zoom in/out (debounced 100ms)
- **Pan**: Touch drag for seeking
- **Scroll/wheel**: Not used for seeking (prevents accidental seeks)

---

## 8. Import Sources

| Code | Source |
|------|--------|
| -4 | Serato 4 |
| -3 | Serato 3 |
| -2 | Rekordbox 5 |
| -1 | Rekordbox 6/7 |
| 1 | Local files |
| 3 | Traktor Pro |
| 4 | VirtualDJ |
| 5 | Serato |
| 6 | EngineDJ |
| 7 | iTunes |
| 8 | M3U playlist |
| 9 | Lexicon |
| 10 | djay |
| 11 | Pioneer USB |

### Streaming Services
Beatport LINK, Beatsource LINK, SoundCloud, Tidal, Spotify, Deezer, Apple Music, iTunes

---

## 9. Speed Comparison: Lexicon vs Rekordbox

| Factor | Lexicon | Rekordbox (estimated) |
|--------|---------|----------------------|
| **Sample rate for analysis** | 12kHz (waveform), 200Hz effective (tempo) | Likely full 44.1kHz |
| **Audio preprocessing** | 7-stage offline render (single pass) | Unknown |
| **BPM algorithm** | Onset + autocorrelation (simple) | Likely more complex |
| **Waveform FFT** | 128-point, 256 samples/segment | Unknown |
| **Parallelism** | Web Worker pool, N concurrent tracks | Unknown |
| **Zero-copy transfer** | Transferable ArrayBuffers | N/A |
| **Key detection** | Custom Krumhansl-Schmuckler, 30% slice | Unknown |
| **No ML/neural nets** | All traditional DSP | Unknown |
| **Language** | JavaScript (V8 JIT, near-native speed) | C++ (native) |

**Bottom line:** Lexicon is fast because it **processes dramatically less data** (12kHz / 200Hz instead of 44.1kHz), uses **simple algorithms** (no ML), and runs **multiple tracks in parallel** via a worker pool. The V8 JIT compiler is fast enough that JavaScript performance is not the bottleneck — it's the data reduction that matters.

---

## 10. No External Libraries for Audio Analysis

The only external dependency in the analysis workers is **Lodash** (utility functions). All DSP is custom-written:

- **Custom FFT**: Hand-rolled Cooley-Tukey in module `W` (Worker 182)
- **Custom FFT library in Worker 160**: Includes DFT, FFT, RFFT, Biquad, IIR filters, window functions — all inlined from what appears to be `dsp.js` / `fft.js`
- **No WASM**: Everything is pure JavaScript
- **No ML models**: All traditional signal processing

The waveform worker also bundles a complete DSP library with oscillators, ADSR envelopes, biquad filters, delay lines, reverb, and graphical EQ — but most of these are unused for waveform generation (they're part of the `dsp.js` library that was bundled wholesale).

---

## 11. Key Takeaways for Pioneer USB Writer

1. **BPM Detection**: Custom onset detection + autocorrelation. The cascading 7-stage lowpass (800→400→400→200→200→200→200 Hz) is the critical preprocessing step that makes it work well on electronic music.

2. **Key Detection**: Standard Krumhansl-Schmuckler with known profiles. The FIR lowpass + downsampling + FFT + triangular kernel binning is straightforward to implement.

3. **Waveform**: 128-point FFT, 3-band energy mapping (Low 0-150Hz, Mid 150-1500Hz, High 1500-Nyquist), 256 samples per segment. Stored as WebP overview + per-segment min/max/color data.

4. **Speed secret**: 12kHz sample rate for waveform, 200Hz effective for tempo. This is the #1 optimization.

5. **Beat grid correction is the hardest part**: Most complexity (~1800 lines) is around finding the correct first beat position. Multiple fallback strategies, dynamic onset refinement, walk-back algorithms.

6. **All local DSP**: No ML models, no WASM, no neural networks. Everything is traditional signal processing running in Web Workers with V8 JIT compilation.

7. **Energy is multi-feature**: 50% RMS + 30% tempo + 50% transient density, with a 0.2× penalty if no strong beats detected.

8. **Cue points are phrase-based**: Energy segmentation into high/low sections, not AI-driven. Emergency loop selection uses energy+beat pattern similarity matching.
