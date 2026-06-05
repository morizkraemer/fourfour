# Player Strip — Implementation Plan

_Last updated: 2026-06-04_

## Goal

Wire the bottom **player strip** (`PlayerCompact` + `app/modules/Player.svelte`) with:

1. **Real waveform** — RGB spectral bars from stored analysis (same scaling as legacy test UI).
2. **Selection sync** — single selected track loads metadata into the player.
3. **Playback** — play/pause, scrub seek, playhead driven by a Rust audio engine (none exists today).

## Discovery summary

### Waveform drawing (already in repo)

| Location | What |
|----------|------|
| `pioneer-test-ui/frontend/app.js:815–1045` | **Reference renderer**: color / mono / peaks modes, zoom 1–64×, pan, beat + time grid. Color bars use `{ amp, r, g, b }` with `rgb(r*255, …)`. |
| `pioneer-test-ui/src/main.rs:get_analysis_data` | Builds `waveform_color` from `ColorWaveform.detail` (150/sec) or `overview` (1200), normalizes bands with `scale = 1/max(low,mid,high)`, `amp = max/96`. |
| `analysis/.../waveform.py` | Python source of truth for 3-band FFT (bass=R, mid=G, high=B). |
| `prototyping/.../waveform.js` + `Waveform.svelte` | **Mini strip** only: mono peak bars + cue dots; no RGB. |
| `PlayerCompact.svelte` | **Placeholder**: CSS gradient + DOM playhead/cues — replace with canvas. |
| `docs/ui-architecture.md` | Target: framework-agnostic `WaveformRenderer` class on Canvas 2D. |

**Decision:** Extract color + mono draw paths from `app.js` into `waveform-renderer.ts` (design-system). Player strip uses **color** when `waveform_color` exists, else **mono** from 400-byte preview.

### Playback engine

| Location | What |
|----------|------|
| Test UI (`frontend/`) | **No playback** — waveform on double-click only (`showWaveform`). |
| `app/stores/player.svelte.ts` | `playing` / `progress` toggle only; `track` always `null`. |
| Workspace Rust | **No** `symphonia`/`rodio`/`cpal` in active `.rs` (old `src/analyzer/` removed; analysis is Python subprocess). |

**Decision:** Add `pioneer-test-ui/src/playback.rs` using **rodio** (OutputStream + Sink + symphonia-backed decoders). Rationale:

- Desktop-native, small dependency surface, fits Tauri command model.
- Decode path aligns with future Rust analyzer revival; no FFmpeg subprocess for preview.
- v1 position: sample-clock from sink + track duration (emit `playback-tick` ~20 Hz); seek = stop + reopen + `try_seek` / skip.

Alternatives considered:

- **Web Audio API in WebView** — fragile path access, duplicate decode, worse for large FLAC.
- **Python subprocess** — same latency issues as analyze; overkill for preview.
- **cpal + symphonia manual** — more control later; rodio is enough for strip MVP.

## Architecture

```
selection (1 track) ──► loadPlayerTrack() ──► get_analysis_data
                              │
                              ▼
                     player.track + waveformColor
                              │
         play/pause/scrub ◄───┴──► Tauri: playback_play / pause / seek / stop
                              │         emit playback-tick { position_ms, playing }
                              ▼
                     WaveformRenderer (canvas in PlayerCompact)
```

## Phases & checkpoints

### CP1 — Plan (this file)
- Document findings and phases.

### CP2 — Waveform renderer ✅
- [x] `waveform-renderer.ts`, `WaveformStrip.svelte`, `PlayerCompact` canvas swap, `svelte.js` export.

### CP3 — Selection → player ✅
- [x] `player.svelte.ts` + `Player.svelte` `$effect` on `selectedTrack()`.

### CP4 — Rust playback ✅
- [x] `playback.rs` + rodio/symphonia + `playback-tick` event.

### CP5 — Frontend playback wiring ✅
- [x] `playback.svelte.ts` + store transport + waveform scrub.

## Out of scope (follow-ups)

- Expanded player (`player-expanded.js` artboard).
- Sample-accurate cue / quantize.
- LRU cache for full `waveform_color` arrays (lazy load per selected track only in v1).

## Verify

```bash
cd app && npm run build
cd pioneer-test-ui && cargo build
./dev.sh  # or cargo tauri dev
```

Manual:

1. Import + analyze a track.
2. Select track → strip shows title + RGB waveform.
3. Play → playhead moves; pause holds; click waveform seeks.
4. Select another track → transport stops, new waveform loads.
