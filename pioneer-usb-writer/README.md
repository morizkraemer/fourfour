# pioneer-usb-writer

A Rust library for reading and writing Pioneer CDJ-compatible USB drives — without Rekordbox.

Produces both the legacy **DeviceSQL** (`export.pdb`) and the newer **OneLibrary** (`exportLibrary.db`) database formats, along with ANLZ analysis files and artwork. Compatible with CDJ-3000, CDJ-3000X, XDJ-AZ, OPUS-QUAD, and older CDJ/XDJ hardware.

## Overview

This is a **format-only library**. It handles:

- Scanning audio files for metadata (tags, artwork, duration, bitrate)
- Writing the complete Pioneer USB directory structure
- Reading back existing USB contents from OneLibrary databases

It does **not** perform audio analysis (BPM detection, key detection, beat grids, waveforms). You bring your own analyzer and populate the provided types.

## Usage

```rust
use pioneer_usb_writer::{scanner, writer, models};
use std::path::Path;

// 1. Scan audio files for metadata
let tracks = scanner::scan_directory(Path::new("/path/to/music"))?;

// 2. Produce analysis results with your own analyzer
let analyses: Vec<models::AnalysisResult> = tracks.iter()
    .map(|track| my_analyzer::analyze(&track.source_path))
    .collect();

// 3. Define playlists
let playlists = vec![
    models::Playlist {
        id: 1,
        name: "My Set".into(),
        track_ids: vec![1, 2, 3],
    },
];

// 4. Write to USB
writer::filesystem::write_usb(
    Path::new("/Volumes/USB"),
    &tracks,
    &analyses,
    &playlists,
)?;
```

### Reading USB contents

```rust
use pioneer_usb_writer::writer::onelibrary;

if let Some(state) = onelibrary::read_usb_state(Path::new("/Volumes/USB"))? {
    for track in &state.tracks {
        println!("{} — {}", track.artist, track.title);
    }
    for playlist in &state.playlists {
        println!("Playlist '{}': {} tracks", playlist.name, playlist.track_ids.len());
    }
}
```

## Analysis Interface

The writer expects an `AnalysisResult` for each track. Populate these fields with your analyzer of choice:

```rust
models::AnalysisResult {
    bpm: 128.0,                          // BPM as f64
    key: "5A".into(),                    // DJ notation (1A-12A, 1B-12B)
    beat_grid: models::BeatGrid {
        beats: vec![
            models::Beat {
                bar_position: 1,          // 1-4 (downbeat = 1)
                time_ms: 500,             // milliseconds from track start
                tempo: 12800,             // BPM * 100
            },
            // ...
        ],
    },
    waveform: models::WaveformPreview {
        data: [0u8; 400],                // 400 bytes, each: 5-bit height | 3-bit whiteness
    },
    cue_points: vec![
        models::CuePoint {
            hot_cue_number: 0,            // 0 = memory cue, 1 = hot cue A, 2 = B, ...
            time_ms: 60000,               // position in milliseconds
            loop_time_ms: None,           // Some(ms) for loops
        },
    ],
}
```

## USB Structure

The writer creates this directory layout:

```
/Volumes/USB/
├── Contents/
│   └── {Artist}/
│       └── track.mp3
├── PIONEER/
│   ├── rekordbox/
│   │   └── export.pdb              # Legacy DeviceSQL database
│   ├── USBANLZ/
│   │   └── P{xxx}/{hash}/
│   │       ├── ANLZ0000.DAT        # Beat grid, waveform preview, cues
│   │       └── ANLZ0000.EXT        # Color waveforms, extended beat grid
│   ├── Artwork/
│   │   └── 00001/
│   │       ├── a{id}.jpg           # 80x80 thumbnail
│   │       └── a{id}_m.jpg         # 240x240 medium
│   └── rekordbox/
│       └── exportLibrary.db        # OneLibrary SQLCipher database
```

## Database Formats

### Legacy PDB (`export.pdb`)

Binary DeviceSQL format with 4096-byte pages. Contains 20 table types covering tracks, artists, albums, genres, labels, keys, colors, playlists, columns, artwork, and history. Supports multi-page tables for large collections.

### OneLibrary (`exportLibrary.db`)

SQLCipher-encrypted SQLite database with 22 tables. Used by CDJ-3000X, XDJ-AZ, OPUS-QUAD, and newer firmware on CDJ-3000. The encryption key is Pioneer's standard obfuscation key (not user-configurable).

Both formats are written on every sync to maximize hardware compatibility.

## Known Limitations

- **Color waveforms** are placeholder (solid green) — no spectral analysis yet
- **Album-artist mapping** uses the first artist encountered per album; compilations show a single artist
- **No incremental sync** — the full database is rewritten each time
