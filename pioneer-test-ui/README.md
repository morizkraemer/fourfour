# pioneer-test-ui

Tauri v2 test harness for `pioneer-usb-writer`. Analysis shells out to the Python CLI (`python -m fourfour_analysis analyze --backend deeprhythm_essentia`, see `resolve_python()` in `src/main.rs`). Analysis is moving to the Rust prototype in `prototyping/analysis-packaging` (`docs/analysis/packaging-decision.md`).

## Running

```bash
# Dev mode (starts Python dev server on :1420)
cargo tauri dev

# Build debug .app bundle
./dev.sh
```

## Layout

- **Frontend**: the Svelte app in `../app` (`frontendDist: ../app/dist`); `frontend/` is the legacy plain HTML/JS UI
- **Analyzer**: external Python CLI in `../analysis`; symphonia is used for playback (`src/playback.rs`)
- **UI**: mirrored split view — Library (tracks | playlists) || (playlists | tracks) USB
