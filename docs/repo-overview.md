# fourfour — Repo Overview

## What This Is

An open-source tool for writing Pioneer CDJ-compatible USB drives. Currently an MVP experimental phase — the format writing works on real CDJ-3000 hardware, but the analysis (BPM, key, beats, waveforms) is still basic and unbenchmarked.

---

## Workspace Structure (3 crates)

```
fourfour/
├── pioneer-usb-writer/    Pure format library — no CLI, no audio analysis
├── pioneer-library/       Persistent local library (SQLite CRUD + USB export/sync)
├── pioneer-test-ui/       Tauri v2 desktop app (the MVP you can click around in)
└── docs/                  Planning docs, not code
```

### `pioneer-usb-writer` (format library)

**What it does:** Reads and writes Pioneer's proprietary binary formats. No audio analysis.

| Module | Lines | Status | What |
|---|---|---|---|
| `writer/pdb.rs` | 1100 | ✅ Works on CDJ-3000 | Legacy DeviceSQL database (`export.pdb`). Multi-page, binary string encoding. |
| `writer/anlz.rs` | 526 | ✅ Works on CDJ-3000 | ANLZ files (`.DAT` + `.EXT`). Beat grids, mono waveforms, color waveforms, cue points, VBR seek tables. |
| `writer/onelibrary.rs` | 906 | ✅ Works on CDJ-3000 | OneLibrary SQLCipher database (`exportLibrary.db`). Required for CDJ-3000X, XDJ-AZ, OPUS-QUAD. |
| `writer/filesystem.rs` | 273 | ✅ | Orchestrator: copy audio, resize artwork, call all writers. Fresh write + incremental sync. |
| `writer/sync.rs` | 349 | ✅ | Diff engine: computes Add/Update/Replace/Skip/Remove actions by comparing tracks vs existing USB. |
| `reader/usb.rs` | 229 | ✅ | Reads existing USB state from OneLibrary `exportLibrary.db`. |
| `reader/masterdb.rs` | 304 | ✅ | Reads Rekordbox local `master.db` (SQLCipher). Returns tracks, cue points, playlists, artwork paths. |
| `reader/anlz.rs` | — | ❌ Missing | ANLZ beat grid reader. Needed for benchmark ground truth. |
| `scanner.rs` | 230 | ✅ | Reads tags via lofty: title, artist, album, genre, artwork, etc. |
| `models.rs` | 234 | ✅ | All shared types: Track, AnalysisResult, BeatGrid, ExistingTrack, SyncPlan, etc. |

**Reference docs:** `reference-code/PIONEER.md` (970 lines of reverse-engineered Pioneer format notes from binary analysis + hardware testing).

### `pioneer-library` (persistent library)

**What it does:** SQLite-backed local music library. CRUD for tracks, analyses, artwork, playlists. Convenience methods for USB export.

| Method | What |
|---|---|
| `add_track`, `add_tracks`, `update_track`, `remove_track` | Track CRUD |
| `set_artwork`, `get_artwork` | Artwork storage |
| `set_analysis`, `get_analysis` | Analysis storage per track |
| `get_unanalyzed_track_ids` | Find tracks needing analysis |
| `create_playlist`, `set_playlist_tracks` | Playlist management |
| `write_usb` | Fresh export to USB |
| `sync_usb` | Incremental sync to USB (add/update/remove) |
| `import_from_usb` | Import tracks from an existing Pioneer USB |
| `import_from_masterdb` | Import from Rekordbox local database |

~1,280 lines total (`lib.rs` 678 + `queries.rs` 498 + `schema.rs` 104).

### `pioneer-test-ui` (Tauri app)

**What it does:** Desktop app you can actually use. Scan folders → analyze tracks → sync to USB.

| Component | Lines | What |
|---|---|---|
| `src/main.rs` | 688 | 15 Tauri commands + app setup |
| `src/analyzer/` | 191 | stratum-dsp BPM/key + symphonia decode → AnalysisResult |
| `frontend/` | 1185 | Vanilla HTML/JS/CSS, mirrored split layout |
| `src/dto.rs` | 67 | Frontend DTOs |

**Tauri commands:** `scan_directory`, `scan_files`, `analyze_tracks`, `write_usb`, `remove_tracks`, `set_test_cues`, `get_mounted_volumes`, `eject_volume`, `wipe_usb`, `read_usb_state`, `save_state`, `load_state`, `get_library_path`, `change_library_path`, `app_version`.

**UI layout:** Library panel (tracks + playlists) on left, USB contents panel on right. Both show track lists with metadata.

---

## What Actually Works on Hardware

The following have been tested on a **real CDJ-3000** (firmware 3.19):

- ✅ PDB database reads correctly (track list, metadata, playlists)
- ✅ OneLibrary database reads correctly
- ✅ ANLZ beat grids play on the CDJ waveform display
- ✅ ANLZ path hash is correct (CDJ finds ANLZ files)
- ✅ Artwork displays on CDJ screen
- ✅ Audio files play from USB
- ✅ Cue points work (hot cues + memory cues)
- ✅ Incremental sync (add/update/remove tracks without rewriting everything)

---

## What Doesn't Work Yet

| Gap | Severity | Notes |
|---|---|---|
| **Color waveforms** (PWV3/PWV4/PWV5) | Medium | Currently hardcoded green. Need FFT-based spectral splitting. |
| **BPM accuracy** | Unknown | stratum-dsp gives numbers but no benchmark vs Rekordbox exists |
| **Key accuracy** | Unknown | Same — works, but untested against ground truth |
| **Beat grid precision** | Unknown | Grids play on CDJ, but no offset measurement vs Rekordbox |
| **PDB multi-page** | Solved | Multi-page support exists in current pdb.rs |

---

## Docs (planning, not code)

| File | Lines | What |
|---|---|---|
| `docs/tech-stack-reference.md` | 391 | Survey of open-source DJ analysis tools (Essentia, madmom, OpenKeyScan, CLAP, Demucs, MSAF). Options + pain points per layer. |
| `docs/experimentation-path.md` | 654 | 6-phase experimentation plan. Phase 0-3 are blocking (benchmark → BPM/key accuracy → waveforms → scale). Phase 4-6 are incremental (phrases → embeddings → stems). |
| `docs/benchmark-implementation-plan.md` | 788 | Concrete build plan for a Python benchmark harness that compares analysis backends against Rekordbox ground truth (from `master.db`). |

---

## Data Flow (current MVP)

```
User scans folder
       │
       ▼
scanner::scan_directory()  ──▶  Vec<Track>  (tags via lofty)
       │
       ▼
Stored in LocalLibrary (SQLite)
       │
       ▼
User clicks "Analyze"
       │
       ▼
analyzer::analyze_track()  ──▶  AnalysisResult (stratum-dsp: BPM, key, beats, waveform)
       │
       ▼
Stored in LocalLibrary
       │
       ▼
User clicks "Sync to USB"
       │
       ▼
sync_usb()  ──▶  diff vs existing USB  ──▶  compute_sync_plan()
       │                                           │
       ▼                                           ▼
write_usb()                            Add/Update/Replace/Skip/Remove
       │
       ├── copy audio to /Contents/{artist}/{file}
       ├── write ANLZ files (beat grid, waveform, cues)
       ├── write export.pdb (legacy DeviceSQL)
       ├── write exportLibrary.db (OneLibrary SQLCipher)
       └── write artwork thumbnails
```

---

## Planned Experiments (separate from the format library)

The benchmark harness is a **separate Python package** (`analysis/`) that:

1. Calls the Rust analyzer as a subprocess (`analyze --json`)
2. Compares output against ground truth from Rekordbox's `master.db` + ANLZ files
3. Runs multiple analysis backends (stratum-dsp, essentia, madmom, etc.)
4. Produces accuracy scorecards per backend

This is documented in `docs/benchmark-implementation-plan.md` and `docs/experimentation-path.md`. **None of this code exists yet** — only the planning docs.

---

## Line Counts

| Component | Lines |
|---|---|
| `pioneer-usb-writer` (Rust) | 3,857 |
| `pioneer-library` (Rust) | 1,280 |
| `pioneer-test-ui` (Rust) | 990 |
| `pioneer-test-ui` (HTML/JS/CSS) | 1,185 |
| Reference code (Rust, read-only) | ~800 |
| Planning docs (Markdown) | 1,833 |
| **Total** | **~9,945** |

---

## Version

`0.9.0` — defined in `pioneer-usb-writer/src/lib.rs` as `pub const VERSION`. Bumped on every change.
