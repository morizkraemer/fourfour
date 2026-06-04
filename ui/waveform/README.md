# WaveformDisplay

Self-contained, zero-dependency DJ waveform renderer using HTML Canvas. No framework, no build step — import the ES module and go.

## Quick Start

```html
<div id="waveform" style="width:100%;height:200px;"></div>

<script type="module">
  import WaveformDisplay from './WaveformDisplay.js';

  const display = new WaveformDisplay(document.getElementById('waveform'));

  display.setData({
    waveform_color: [
      { amp: 0.8, r: 30, g: 100, b: 255 },
      { amp: 0.6, r: 255, g: 140, b: 0 },
      // ... one entry per waveform column (~150/sec)
    ],
    waveform_preview: new Uint8Array(400), // Pioneer PWAV fallback (optional if waveform_color provided)
    beats: [
      { time_ms: 0, bar_position: 1 },
      { time_ms: 500, bar_position: 2 },
      // ...
    ],
    duration_ms: 300000,
    bpm: 128.0,
  });
</script>
```

## Data Shape

| Field | Type | Description |
|---|---|---|
| `waveform_color` | `Array<{amp, r, g, b}>` | 3-band color waveform detail. `amp` 0–1, `r/g/b` 0–255. ~150 columns/sec. Used for the zoom view. |
| `waveform_overview` | `Array<{amp, r, g, b}>` | 1200-entry overview waveform (Pioneer PWV6 format). Used for the overview bar. Falls back to downsampling `waveform_color` if absent. |
| `waveform_preview` | `Uint8Array \| number[]` | 400-byte Pioneer PWAV. 5-bit height + 3-bit whiteness per byte. Used as monochrome fallback when `waveform_color` is absent. |
| `beats` | `Array<{time_ms, bar_position}>` | Beat grid. `bar_position` 1–4 within each bar. |
| `duration_ms` | `number` | Track duration in milliseconds. |
| `bpm` | `number` | Tempo. Displayed as a pill on the first downbeat. |

## API

### `new WaveformDisplay(container)`

Creates the component inside `container`. Appends two canvases: an overview (full-track) and a zoom view (scrollable detail).

### `setData(data)`

Set waveform data and render. Resets zoom and scroll.

### `clear()`

Clear to empty state.

### `redraw()`

Re-render at current zoom/scroll. Call after the container resizes.

### `setPlayhead(frac)`

Set playhead position as a 0–1 fraction of duration. Pass -1 to hide.

### `setViewport(zoom, offset)`

Programmatically set zoom level (1–512) and scroll offset (0–1). Useful for syncing multiple displays.

### `destroy()`

Remove canvases and clean up event listeners.

### Callbacks

| Callback | Signature | Fires when |
|---|---|---|
| `onViewportChange` | `(zoom, offset) => void` | User drags/scrolls the zoom view |
| `onSeek` | `(frac) => void` | User clicks to seek (overview or zoom view) |

```js
display.onViewportChange = (zoom, offset) => {
  otherDisplay.setViewport(zoom, offset); // sync two displays
};

display.onSeek = (frac) => {
  audio.currentTime = frac * audio.duration;
};
```

## Interaction

- **Drag horizontal** — pan
- **Drag vertical** — zoom in/out
- **Scroll wheel** — pan (when zoomed)
- **Click overview** — jump to position
- **Click zoom** — seek to position

## Rendering

Two views are always rendered:

1. **Overview** — stacked bars from bottom (bass, mid, high). Shows a viewport indicator for the current zoom region.
2. **Zoom** — mirrored filled shapes (symmetric around center). Includes beat grid with bar numbers, phrase markers (every 4 bars bold), time grid, and BPM pill.

Falls back to monochrome rendering from `waveform_preview` when `waveform_color` is not provided.

## Dev Harness

The `dev/` subfolder contains a development harness for comparing waveform output against Rekordbox:

```bash
shed run wv <audio-file>       # analyze + open side-by-side comparison
shed run wv-hood               # quick run with reference track
```

This runs `waveform_dev` which analyzes audio via the Python `fourfour_analysis` package, writes `dev/data.json`, reads Rekordbox ANLZ data into `dev/rekordbox.json`, and serves `dev/dev.html` on localhost.

## Integration Guide: Rust App + Python Analysis + WaveformDisplay

How to get a working waveform in a new Rust application (e.g. Tauri) using the `fourfour_analysis` Python backend and `WaveformDisplay.js`.

### 1. Run the Python analyzer from Rust

The `fourfour_analysis` package produces all waveform data. Shell out to it with `--json`:

```rust
let output = std::process::Command::new("path/to/analysis/.venv/bin/python")
    .args(["-m", "fourfour_analysis", "analyze", audio_path, "--backend", "deeprhythm_essentia", "--json"])
    .output()?;

let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
// Unwrap { "deeprhythm_essentia": { ...result... } }
let result = json.as_object().unwrap().values().next().unwrap();
```

The result contains everything you need:

| JSON field | What it is |
|---|---|
| `bpm` | Detected BPM (float) |
| `key` | Musical key in Camelot notation (`"8A"`) |
| `beats` | `[{time_seconds, bar_position}]` — beat grid |
| `waveform_colors` | `[{r, g, b}]` — 3-band detail, ~150 cols/sec, values 0–127 |
| `waveform_overview` | `[{r, g, b}]` — 1200-entry overview, values 0–127 |
| `waveform_preview` | `[int]` — 400-byte Pioneer PWAV (height + whiteness per byte) |
| `waveform_peaks` | `[{min_val, max_val}]` — raw amplitude per column |

### 2. Transform for WaveformDisplay

The JS component expects `{amp, r, g, b}` per column. Convert the Python output:

```rust
fn to_display_format(colors: &[serde_json::Value]) -> Vec<serde_json::Value> {
    colors.iter().map(|c| {
        let r = c["r"].as_f64().unwrap_or(0.0);
        let g = c["g"].as_f64().unwrap_or(0.0);
        let b = c["b"].as_f64().unwrap_or(0.0);
        // Amplitude from max band value (filterbank output is 0–127)
        let max_ch = r.max(g).max(b).max(1.0);
        let amp = (max_ch / 127.0).min(1.0);
        // Normalize colors so dominant band = 255
        let scale = 255.0 / max_ch;
        serde_json::json!({
            "amp": amp,
            "r": (r * scale).min(255.0) as u8,
            "g": (g * scale).min(255.0) as u8,
            "b": (b * scale).min(255.0) as u8,
        })
    }).collect()
}
```

Build the display data object:

```rust
let beats: Vec<serde_json::Value> = result["beats"].as_array().unwrap()
    .iter()
    .map(|b| serde_json::json!({
        "time_ms": (b["time_seconds"].as_f64().unwrap() * 1000.0) as u64,
        "bar_position": b["bar_position"].as_u64().unwrap(),
    }))
    .collect();

let colors = result["waveform_colors"].as_array().unwrap();
let overview = result["waveform_overview"].as_array().unwrap();
let duration_ms = (colors.len() as f64 * 80.0 / 12_000.0 * 1_000.0) as u64;

let display_data = serde_json::json!({
    "waveform_color": to_display_format(colors),
    "waveform_overview": to_display_format(overview),
    "waveform_preview": result["waveform_preview"],
    "beats": beats,
    "duration_ms": duration_ms,
    "bpm": result["bpm"],
});
```

### 3. Wire up the frontend

Copy `WaveformDisplay.js` into your frontend assets. In your HTML:

```html
<div id="waveform" style="width:100%;height:300px;background:#0d0d0d;"></div>

<script type="module">
  import WaveformDisplay from './WaveformDisplay.js';

  const display = new WaveformDisplay(document.getElementById('waveform'));

  // Fetch display_data from your Rust backend (Tauri invoke, HTTP endpoint, etc.)
  const data = await fetchWaveformData();
  display.setData(data);

  // Optional: audio playback sync
  const audio = document.getElementById('audio-player');
  display.onSeek = (frac) => { audio.currentTime = frac * audio.duration; };

  // Update playhead during playback
  function tick() {
    if (!audio.paused) {
      display.setPlayhead(audio.currentTime / audio.duration);
    }
    requestAnimationFrame(tick);
  }
  tick();
</script>
```

### Known Limitations

The waveform color data and preview are production-ready. However:

- **Beat grid is not accurate yet.** The beat detection (phase alignment, downbeat detection) is still being worked on. Expect bar lines to drift or land on wrong beats. Don't rely on beat/bar positions for anything precise.
- **Cue points are experimental.** Section detection is basic and will produce incorrect results on many tracks.
- **Key detection** works but has ~54% exact accuracy (Essentia bgate). Good enough for display, not for harmonic mixing logic.

The waveform display will look correct regardless — beat grid inaccuracy only affects the grid overlay lines and bar numbers, not the waveform shape or colors.

### Prerequisites

The Python analysis environment needs:
- Python 3.10+ with a venv at `analysis/.venv/`
- `pip install -e analysis/` (installs `fourfour_analysis`)
- Heavy deps: `torch`, `DeepRhythm`, `essentia`, `numpy`, `scipy`

The `deeprhythm_essentia` backend gives you: DeepRhythm BPM (best-in-class ML tempo), Essentia key detection, and Rekordbox-calibrated 3-band color waveforms via Butterworth crossover filters.
