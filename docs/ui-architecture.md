# UI Architecture — Pioneer App

Decision document for the production UI that will replace Rekordbox. Covers framework choices, rendering strategy, and waveform architecture.

---

## 1. App Shell: Tauri

**Decision: Keep Tauri.**

Tauri gives us a Rust backend (our library is already Rust) with a web-based frontend rendered in the platform's native WebView. The key advantages over alternatives:

| Option | Bundle size | Performance | Rust integration | Talent pool | Verdict |
|--------|------------|-------------|-----------------|-------------|---------|
| **Tauri** | ~10-20 MB | Near-native via WebView | Native (Tauri commands call Rust directly) | Huge (web devs) | **Chosen** |
| Flutter | ~30-50 MB | GPU-rendered, 120fps capable | FFI bindings required | Moderate | Good but adds Dart, needs FFI |
| Electron | 200+ MB | Chromium overhead | Node.js native addons | Huge | Too heavy, opposite of our goals |
| Two native apps (SwiftUI + WinUI) | Minimal | Best possible per-platform | Swift + C# bindings | Small per-platform | 2x maintenance forever |
| Rust-native UI (Iced, GPUI, Slint) | Minimal | GPU-rendered | Same language | Very small | Immature ecosystems, limited widgets |

**Why Tauri over native:**
- Cross-platform requirement eliminates SwiftUI-only
- Web rendering gap has closed — Linear, Figma, Spotify prove web can feel premium
- Rust library integrates with zero FFI overhead
- Bundle stays lightweight (~10-20 MB vs Rekordbox's multi-GB install)
- Web dev talent pool is 100x larger than any alternative

**What makes a Tauri app feel native:**
- 60fps transitions with no layout jank
- Instant feedback on all interactions
- CSS transitions / animations (not JS-driven)
- Proper keyboard shortcuts and OS-native context menus
- No visible loading states for local operations

---

## 2. Frontend Framework: React or Svelte

Both are strong choices. The decision should be driven by the UI developer's preference.

### React

**Pros:**
- Largest ecosystem — any component library exists
- Best AI copilot support (every model has trained on millions of React components)
- Most developers already know it
- Virtualized lists via `@tanstack/virtual` are battle-tested
- State management: Zustand or Jotai are lightweight and clean

**Cons:**
- Virtual DOM adds overhead that's unnecessary in a desktop app (no SSR)
- More boilerplate: `useState`, `useEffect`, `useMemo`, `useCallback`
- Larger JS bundle than compiled frameworks

### Svelte 5 (alternative worth considering)

**Pros:**
- No virtual DOM — direct DOM updates, less overhead for reactive lists/waveforms
- Smallest JS bundle (compiles away the framework)
- Less boilerplate — reactivity built into the language
- First-class Tauri template support
- AI support is solid (simpler language surface = fewer model mistakes)

**Cons:**
- Smaller ecosystem than React
- Fewer developers know it
- Some niche component libraries may not exist

### Recommendation

If the UI developer is a React dev, go React. If starting fresh with no preference, Svelte 5 has technical advantages for this specific use case (performance-sensitive desktop app with lots of reactive state). Either will produce a great result.

---

## 3. Waveform Rendering

The waveform is the most important and most interactive UI element. It needs to support:
- Static display of the full track waveform
- Scrolling waveform during playback (real-time, 60fps)
- Scrubbing (drag to seek)
- Beat grid overlay (vertical lines at each beat)
- Cue marker display and dragging (repositioning cues on the waveform)
- Zoom in/out

### Rendering technology: Canvas 2D

Canvas 2D is the right choice for all waveform rendering in this app. The data scale does not justify WebGL/WebGPU.

**Why not WebGL/WebGPU:**
- WebGL only becomes worthwhile for: full spectrograms (thousands of frequency bins x thousands of time slices), 3D extruded waveforms with lighting, or 100k+ simultaneously visible points with real-time transforms
- Our waveform draws at most ~2000 bars per frame — Canvas 2D handles this in <0.5ms
- WebGL adds significant complexity (shaders, buffer management, state machines) with zero performance gain at this scale
- Text rendering, cue marker labels, and overlay elements are much simpler in Canvas 2D

**When to revisit:** If we add a full spectrogram view (frequency-domain visualization), that would be a WebGL candidate. The waveform itself stays Canvas 2D.

### Waveform data

Currently the library produces these waveform formats (written to ANLZ files for CDJ hardware):

| Tag | Resolution | Per entry | Purpose |
|-----|-----------|-----------|---------|
| PWAV | 400 points (fixed) | 1 byte | Monochrome preview for CDJ screen |
| PWV3 | `duration_secs * 150` | 1 byte | Color preview for CDJ |
| PWV5 | `duration_secs * 150` | 2 bytes | Color detail for CDJ |
| PWV4 | 1200 points (fixed) | 6 bytes (RGB) | Color waveform for CDJ |

**Problem:** All of these are currently derived from the 400-byte PWAV data. The color is faked (hardcoded). This is fine for CDJ hardware but not for our app UI.

**What we need for the app:** Real high-resolution waveform data with spectral color, generated from the actual audio signal. This means:

1. **In the analyzer:** Compute waveform at ~150 points/sec or higher, with real frequency band data:
   - Low frequencies (bass/kick) -> red/warm
   - Mid frequencies (vocals/synths) -> green
   - High frequencies (hats/cymbals) -> blue
   
2. **New field in `AnalysisResult`:** Something like:
   ```rust
   pub struct WaveformDetail {
       pub samples: Vec<WaveformSample>,
       pub samples_per_second: f64,
   }
   
   pub struct WaveformSample {
       pub low: u8,    // bass amplitude (red channel)
       pub mid: u8,    // mid amplitude (green channel)
       pub high: u8,   // treble amplitude (blue channel)
       pub amplitude: u8, // overall height
   }
   ```

3. **Resolution example:** A 6-minute track at 150 samples/sec = 54,000 points. At 4 bytes each = ~216 KB per track. Trivial in memory.

4. **Level-of-detail for rendering:**
   - Zoomed out (full track visible): downsample to ~1000-2000 points to match pixel width
   - Zoomed in (scrubbing): render from full-res data, only the visible window
   - Downsampling 54k points is sub-millisecond in JS, no need for pre-computed mip levels

### Waveform component architecture

The waveform renderer should be a **framework-agnostic TypeScript module** that takes a `<canvas>` element and data, handles all drawing and interaction internally. The framework (React/Svelte) just mounts it and feeds data.

```
┌─────────────────────────────────────────────────────┐
│                  Canvas Element                      │
│                                                     │
│  Layer 1: Waveform bars ────────────────────────    │
│           (colored by frequency: R=low G=mid B=hi)  │
│                                                     │
│  Layer 2: Beat grid ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─    │
│           (vertical lines at each beat position)    │
│                                                     │
│  Layer 3: Cue markers ▼─────────▼────────▼          │
│           (draggable flags with labels)             │
│                                                     │
│  Layer 4: Playhead │                                │
│           (current position, moves during playback) │
└─────────────────────────────────────────────────────┘
```

All layers draw bottom-up each frame on `requestAnimationFrame`. At 2000 bars + grid lines + cue markers, total draw time is <1ms.

**Suggested API:**

```typescript
interface WaveformOptions {
  colors: { low: string; mid: string; high: string };
  beatGridColor: string;
  cueMarkerStyle: CueMarkerStyle;
  scrollMode: 'static' | 'scrolling';  // static = full track, scrolling = playhead-centered
}

class WaveformRenderer {
  constructor(canvas: HTMLCanvasElement, options: WaveformOptions) {}

  // Data
  setWaveformData(samples: WaveformSample[], samplesPerSecond: number): void;
  setBeatGrid(beats: Beat[]): void;
  setCuePoints(cues: CuePoint[]): void;
  setDuration(durationMs: number): void;

  // Playback
  setPlayhead(positionMs: number): void;  // call on rAF during playback
  setScrollMode(mode: 'static' | 'scrolling'): void;

  // Zoom
  setZoom(level: number): void;  // 1.0 = full track visible
  setViewRange(startMs: number, endMs: number): void;

  // Interaction callbacks
  onScrub(callback: (positionMs: number) => void): void;
  onCueMove(callback: (cueId: number, newPositionMs: number) => void): void;
  onCueCreate(callback: (positionMs: number) => void): void;

  // Lifecycle
  resize(): void;   // call on container resize
  destroy(): void;  // cleanup listeners, cancel rAF
}
```

### Interaction model

All interaction is via pointer events on the canvas:

- **Scrubbing:** `pointerdown` + `pointermove` on empty waveform area -> seek to position
- **Cue dragging:** `pointerdown` near a cue marker (hit test: pointer X within ~5px of cue position) -> drag mode, `pointermove` updates cue position, `pointerup` commits
- **Cue creation:** Double-click or right-click context menu at position
- **Beat grid snapping:** When dragging cues, optionally snap to nearest beat position (quantize to beat grid)
- **Zoom:** Scroll wheel or pinch gesture changes zoom level, centered on pointer position

### Scrolling waveform (playback mode)

During playback, the waveform scrolls so the playhead stays at a fixed screen position (e.g., 1/3 from left). Implementation:

1. On each `requestAnimationFrame`, compute visible time window based on playhead position and zoom level
2. Determine which waveform samples fall in the visible window
3. Draw only those samples, offset so the playhead aligns to the fixed position
4. Draw beat grid lines and cue markers that fall in the visible window

This is still Canvas 2D — we're drawing the same ~1000-2000 bars per frame, just shifting which ones. The per-frame cost is constant regardless of track length.

**Performance budget:** At 60fps we have 16.6ms per frame. Waveform drawing takes <1ms. The remaining 15ms is plenty for framework reactivity, other UI updates, and browser overhead.

---

## 4. Bundle & Performance Budget

Target metrics for the production app:

| Metric | Target | Why |
|--------|--------|-----|
| App bundle (DMG/installer) | < 30 MB | Rekordbox is ~2 GB. Our lightness is a feature. |
| Cold launch to interactive | < 2 seconds | Must feel instant. Tauri + WebView is fast. |
| Library scan (1000 tracks) | < 30 seconds | Metadata reading is I/O-bound, parallelize. |
| Waveform render (per frame) | < 2 ms | Leaves 14ms headroom at 60fps. |
| Track list scroll | 60 fps | Use virtualized list (only render visible rows). |
| Memory (1000 track library) | < 200 MB | Waveform data: ~216 KB/track = ~216 MB for 1000. Consider lazy loading. |

### Memory consideration for waveform data

At 216 KB per track, a 1000-track library's waveform data alone is ~216 MB. Options:
- **Lazy load:** Only load waveform detail for visible/selected tracks. Keep lightweight preview data (400 bytes) for all tracks.
- **Disk cache:** Store computed waveform data alongside ANLZ files, load on demand.
- **LRU cache:** Keep last N viewed waveforms in memory, evict oldest.

Recommended: Lazy load with an LRU cache of ~50 waveforms in memory.

---

## 5. Summary of Decisions

| Decision | Choice | Status |
|----------|--------|--------|
| App framework | Tauri | Confirmed |
| Frontend framework | React or Svelte 5 (developer preference) | Open |
| Waveform rendering | Canvas 2D | Confirmed |
| Waveform data source | New high-res spectral data from analyzer | To implement |
| Waveform component | Framework-agnostic TypeScript class | To implement |
| Scrolling waveform | Playhead-centered scroll on rAF | Confirmed |
| WebGL/WebGPU | Not needed (revisit only for spectrogram) | Confirmed |
| State management | Zustand (React) or built-in (Svelte) | Depends on framework |
| List virtualization | @tanstack/virtual (React) or svelte-virtual-list | Depends on framework |
