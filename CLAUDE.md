# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

A Rust library that reads and writes Pioneer CDJ-compatible USB drives without Rekordbox. Monorepo with two crates:

- **`pioneer-usb-writer/`** — Pure format library. Scans audio file metadata, writes the Pioneer USB structure (PDB database, OneLibrary SQLCipher database, ANLZ files, artwork, audio). **No audio analysis** — consumers bring their own BPM/key/waveform analyzer and populate the provided types.
- **`pioneer-test-ui/`** — Throwaway Tauri v2 test harness (vanilla HTML/JS frontend) with a bundled analyzer (stratum-dsp). The real UI will be built separately by a collaborator.

## Build & Run

```bash
# Build everything
cargo build

# Run Tauri test UI (starts Python dev server on :1420)
cargo tauri dev

# Build Tauri debug .app bundle and open it
cd pioneer-test-ui && ./dev.sh
```

No test suite exists yet. Validation is done against real CDJ-3000 hardware using binary bisection (see `pioneer-usb-writer/reference-code/PIONEER.md`).

## Workspace Cargo.toml

Dev builds optimize DSP/audio dependencies at `opt-level = 3` while keeping app code in debug. This is critical — stratum-dsp is 10-50x slower unoptimized. When adding new symphonia sub-crates, add a corresponding `[profile.dev.package.*]` entry.

## Versioning

The app version is defined in **`pioneer-usb-writer/src/lib.rs`** as `pub const VERSION`. It is displayed in the Tauri UI toolbar.

**On every edit or feature change**, bump the version in `pioneer-usb-writer/src/lib.rs`.

## Architecture

### Library crate (`pioneer-usb-writer`)

The library is a **format-only crate** — it handles scanning metadata and writing Pioneer's binary formats. It does not perform audio analysis (BPM detection, key detection, waveform generation). Consumers populate the `AnalysisResult` type with their own analyzer and pass it to the writer.

**Pipeline: scan → (external analysis) → write**

```
scanner::scan_directory()  →  Vec<Track>             (metadata via lofty)
                           ↓
              [consumer fills AnalysisResult]          (BPM, key, beat grid, waveform)
                           ↓
writer::filesystem::write_usb()                       (orchestrates all output)
  ├── copies audio to /Contents/{artist}/{file}
  ├── writer::anlz       → ANLZ0000.{DAT,EXT}        (beat grid, waveforms, cues)
  ├── writer::pdb        → export.pdb                 (legacy DeviceSQL database)
  ├── writer::onelibrary → exportLibrary.db           (OneLibrary SQLCipher database)
  └── artwork JPGs       → /PIONEER/Artwork/          (80x80 + 240x240 thumbnails)
```

**Key modules:**

- **`models.rs`** — Core types shared across all modules:
  - `Track` — metadata from tags (title, artist, album, BPM, key, artwork, etc.)
  - `AnalysisResult` — analysis output: `BeatGrid`, `WaveformPreview`, BPM, key, `CuePoint`s
  - `Playlist` — named list of track IDs
  - `ExistingTrack`, `ExistingPlaylist`, `ExistingUsbState` — types for reading back from USB
- **`scanner.rs`** — Reads tags with lofty. Extracts: title, artist, album, genre, label, remixer, comment, year, disc/track number, artwork bytes. Builds USB-relative paths with sanitized components.
- **`writer/filesystem.rs`** — Orchestration: copy audio, resize artwork, call anlz + pdb + onelibrary writers.
- **`writer/pdb.rs`** (~1080 lines) — Generates the legacy DeviceSQL `export.pdb`. 20 table types, multi-page support, binary string encoding (ASCII + UTF-16LE).
- **`writer/anlz.rs`** (~490 lines) — Generates ANLZ files (.DAT and .EXT). Beat grids, waveform previews, color waveforms, cue points, VBR sections.
- **`writer/onelibrary.rs`** (~1120 lines) — Generates the OneLibrary `exportLibrary.db` (SQLCipher-encrypted SQLite). 22 tables, static lookups + dynamic data.
- **`reader/usb.rs`** — Reads back existing USB OneLibrary state (`read_usb_state()`). Re-exported as `pioneer_usb_writer::reader::read_usb_state`.
- **`reader/masterdb.rs`** — Reads Rekordbox's local `master.db` (SQLCipher key `402fd482...`). Returns `MasterDbImport` with tracks, cue points, playlists, and artwork paths.

### Test UI crate (`pioneer-test-ui`)

- **`src/analyzer/`** — Audio analysis module (stratum-dsp + symphonia). Decodes audio to mono f32, detects BPM/key/beats, generates 400-byte waveform preview. This is the reference analyzer implementation — not part of the library.
- `AppState` holds `Vec<Track>` + parallel `Vec<Option<AnalysisResult>>`, wrapped in `Arc<Mutex<_>>`.
- `analyze_tracks` is async — each track runs via `tokio::task::spawn_blocking` with `catch_unwind` so a panic in DSP code skips the track instead of crashing.
- Frontend is plain HTML/JS in `pioneer-test-ui/frontend/`. No framework. `window.prompt()` doesn't work in Tauri's WKWebView — use `<dialog>` elements instead.
- Tauri commands: `scan_directory`, `scan_files`, `analyze_tracks`, `write_usb`, `pick_directory`, `get_mounted_volumes`, `read_usb_state`, `save_state`, `load_state`.
- UI layout: mirrored split view — Library (tracks | playlists) || (playlists | tracks) USB.

## Using the Library

To use `pioneer-usb-writer` in your own project:

```rust
use pioneer_usb_writer::{reader, scanner, writer, models};

// 1. Scan audio files for metadata
let tracks = scanner::scan_directory(Path::new("/path/to/music"))?;

// 2. Analyze tracks with YOUR analyzer (not included in this crate)
let analyses: Vec<models::AnalysisResult> = tracks.iter()
    .map(|track| your_analyzer::analyze(&track.source_path))
    .collect();

// 3. Define playlists
let playlists = vec![models::Playlist {
    id: 1,
    name: "My Playlist".to_string(),
    track_ids: vec![1, 2, 3],
}];

// 4. Write to USB
writer::filesystem::write_usb(
    Path::new("/Volumes/USB"),
    &tracks,
    &analyses,
    &playlists,
)?;

// 5. Read back USB state (optional)
let state = reader::read_usb_state(Path::new("/Volumes/USB"))?;
```

The `AnalysisResult` struct you need to populate:

```rust
AnalysisResult {
    beat_grid: BeatGrid { beats: Vec<Beat> },  // bar_position (1-4), time_ms, tempo (BPM*100)
    waveform: WaveformPreview { data: [u8; 400] },  // 5-bit height + 3-bit whiteness per byte
    bpm: f64,
    key: String,       // DJ notation: "1A", "5B", etc.
    cue_points: Vec<CuePoint>,
}
```

## Pioneer Format Gotchas

These are hard-won from hardware testing. See `reference-code/PIONEER.md` for full details.

- **Dual database format**: The writer produces both `export.pdb` (legacy DeviceSQL) and `exportLibrary.db` (OneLibrary SQLCipher). Modern CDJs (CDJ-3000X, XDJ-AZ, OPUS-QUAD) prefer OneLibrary; older hardware uses PDB.
- **OneLibrary encryption**: SQLCipher with key `r8gddnr4k847830ar6cqzbkk0el6qytmb3trbbx805jm74vez64i5o8fnrqryqls`. This is Pioneer's standard key (not a security measure — obfuscation only). WAL must be checkpointed after writing.
- **ANLZ path hash**: CDJ computes the USBANLZ directory path from the USB-relative audio path using a specific hash algorithm. If you get this wrong, the CDJ silently regenerates ANLZ files (creates ANLZ0001.DAT alongside your ANLZ0000.DAT — diff them to debug).
- **History tables must be populated**: Even for fresh exports, the CDJ requires non-empty history data pages. Three reference binary blobs are embedded via `include_bytes!()`.
- **PDB page 0 sequence**: Must exceed all data page sequences or CDJ ignores the database.
- **Columns table (0x10)**: Uses a different page header format — `unknown5` = num_rows instead of the usual 0x0001.
- **DeviceSQL string encoding**: Length-prefixed, specific byte markers (0x40 for long strings, 0x90 for UTF-16LE). See `encode_string()` in pdb.rs.
- **PPTH tag in ANLZ**: Path must be null-terminated UTF-16BE, and the tag's `len_path` includes the null terminator bytes.
- **Color waveforms (PWV3/PWV4/PWV5)**: Currently faked with hardcoded green — no spectral analysis yet.
- **Album-artist mapping**: Both PDB and OneLibrary use the first artist encountered for each album. Compilations/VA albums will show a single artist.
