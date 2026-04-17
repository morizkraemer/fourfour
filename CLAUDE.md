# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

A Rust tool that writes Pioneer CDJ-compatible USB drives without Rekordbox. Monorepo with two crates:

- **`pioneer-usb-writer/`** — Library + CLI. Scans audio files, runs BPM/key analysis, writes the Pioneer USB structure (PDB database, ANLZ files, artwork, audio).
- **`pioneer-test-ui/`** — Throwaway Tauri v2 test harness (vanilla HTML/JS frontend). The real UI will be built separately by a collaborator.

## Build & Run

```bash
# Build everything
cargo build

# Run CLI: scan dirs → analyze → write USB
cargo run -p pioneer-usb-writer --bin pioneer-usb-writer -- /path/to/music /another/dir -o /Volumes/USB

# Run Tauri test UI (starts Python dev server on :1420)
cargo tauri dev

# Build Tauri debug .app bundle and open it
cd pioneer-test-ui && ./dev.sh
```

No test suite exists yet. Validation is done against real CDJ-3000 hardware using binary bisection (see `pioneer-usb-writer/reference-code/PIONEER.md`).

## Workspace Cargo.toml

Dev builds optimize DSP/audio dependencies at `opt-level = 3` while keeping app code in debug. This is critical — stratum-dsp is 10-50x slower unoptimized. When adding new symphonia sub-crates, add a corresponding `[profile.dev.package.*]` entry.

## Architecture

The library pipeline is: **scan → analyze → write**.

```
scanner::scan_directory()  →  Vec<Track>        (metadata via lofty)
analyzer::analyze_track()  →  AnalysisResult     (BPM/key via stratum-dsp, decode via symphonia)
writer::filesystem::write_usb()                  (orchestrates all output)
  ├── copies audio to /Contents/{artist}/{file}
  ├── writer::anlz  → /PIONEER/USBANLZ/P{xxx}/{hash}/ANLZ0000.{DAT,EXT}
  ├── writer::pdb   → /PIONEER/rekordbox/export.pdb
  └── artwork JPGs  → /PIONEER/Artwork/00001/{a,b}{id}.jpg
```

### Key modules

- **`models.rs`** — Core types: `Track`, `AnalysisResult`, `BeatGrid`, `WaveformPreview`, `Playlist`. Shared across all modules.
- **`scanner.rs`** — Reads tags with lofty. Extracts: title, artist, album, genre, label, remixer, comment, year, disc/track number, artwork bytes.
- **`analyzer.rs`** — Decodes audio to mono f32 via symphonia, runs `stratum_dsp::analyze_audio()`, converts beat grid to Pioneer format, generates 400-byte waveform preview.
- **`writer/pdb.rs`** (~920 lines) — Generates the DeviceSQL database. 20 table types across 41 pre-allocated 4096-byte pages. Track rows are 344+ bytes with a 94-byte fixed header + 21 string offset slots.
- **`writer/anlz.rs`** (~440 lines) — Generates ANLZ files. The ANLZ path hash is computed independently by the CDJ — the PDB `analyze_path` field is ignored. The hash algorithm is documented in `reference-code/PIONEER.md`.
- **`writer/filesystem.rs`** — Orchestration: copy audio, resize artwork to 80x80 and 240x240, call anlz + pdb writers.

### Tauri test UI

- `AppState` holds `Vec<Track>` + parallel `Vec<Option<AnalysisResult>>`, wrapped in `Arc<Mutex<_>>`.
- `analyze_tracks` is async — each track runs via `tokio::task::spawn_blocking` with `catch_unwind` so a panic in DSP code skips the track instead of crashing.
- Frontend is plain HTML/JS in `pioneer-test-ui/frontend/`. No framework. `window.prompt()` doesn't work in Tauri's WKWebView — use `<dialog>` elements instead.
- Tauri commands: `scan_directory`, `scan_files`, `analyze_tracks`, `write_usb`, `pick_directory`, `get_mounted_volumes`.

## Pioneer Format Gotchas

These are hard-won from hardware testing. See `reference-code/PIONEER.md` for full details.

- **ANLZ path hash**: CDJ computes the USBANLZ directory path from the USB-relative audio path using a specific hash algorithm. If you get this wrong, the CDJ silently regenerates ANLZ files (creates ANLZ0001.DAT alongside your ANLZ0000.DAT — diff them to debug).
- **History tables must be populated**: Even for fresh exports, the CDJ requires non-empty history data pages. Three reference binary blobs are embedded via `include_bytes!()`.
- **PDB page 0 sequence**: Must exceed all data page sequences or CDJ ignores the database.
- **Columns table (0x10)**: Uses a different page header format — `unknown5` = num_rows instead of the usual 0x0001.
- **DeviceSQL string encoding**: Length-prefixed, specific byte markers (0x26 for long strings, 0x90 for short). See `encode_string()` in pdb.rs.
- **PPTH tag in ANLZ**: Path must be null-terminated UTF-16BE, and the tag's `len_path` includes the null terminator bytes.
- **Color waveforms (PWV3/PWV4/PWV5)**: Currently faked with hardcoded green — no spectral analysis yet. This is the next major feature.
- **PDB single-page limit**: Current POC fits all rows of each table in one 4096-byte page. Will need multi-page support for large collections.
