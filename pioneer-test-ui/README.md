# pioneer-test-ui

Throwaway Tauri v2 test harness for `pioneer-usb-writer`. Uses a bundled analyzer (stratum-dsp) as the reference implementation.

## Running

```bash
# Dev mode (starts Python dev server on :1420)
cargo tauri dev

# Build debug .app bundle
./dev.sh
```

## Layout

- **Frontend**: plain HTML/JS in `frontend/` — no framework
- **Analyzer**: `src/analyzer/` — stratum-dsp BPM/key detection + symphonia audio decoding
- **UI**: mirrored split view — Library (tracks | playlists) || (playlists | tracks) USB
