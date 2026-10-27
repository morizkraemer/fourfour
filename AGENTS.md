# AGENTS.md

Agent entrypoint for the **fourfour** repository. Start here, then follow the
[Documentation Map](#documentation-map) into `docs/` for anything deeper.

> `CLAUDE.md` is a thin pointer to this file — this is the single source of
> truth for agent guidance, regardless of which tool you are.

## What This Is

An open-source tool for writing Pioneer CDJ-compatible USB drives without
Rekordbox — scan music, analyze it, and export a drive that plays on real
CDJ hardware. The format library is proven on a CDJ-3000; the desktop UI and
the analysis pipeline are the active frontier.

## Repository Layout

A Cargo workspace (3 Rust crates) plus a Svelte frontend, a Python analysis
CLI, and prototyping/design assets.

| Path | Lang | What |
|---|---|---|
| `pioneer-usb-writer/` | Rust | **Format library.** Scans tags, writes the Pioneer USB structure (PDB, OneLibrary, ANLZ, artwork, audio). No audio analysis — consumers supply `AnalysisResult`. |
| `pioneer-library/` | Rust | **Persistent library.** SQLite-backed CRUD for tracks/analyses/playlists + USB export & incremental sync. |
| `pioneer-test-ui/` | Rust | **Tauri v2 shell.** Hosts the Svelte UI (`frontendDist: ../app/dist`), exposes Tauri commands, and shells out to the Python CLI for analysis (`python -m fourfour_analysis analyze --backend deeprhythm_essentia`; `resolve_python()` in `src/main.rs`). Analysis is moving to the Rust prototype in `prototyping/analysis-packaging` (`docs/analysis/packaging-decision.md`). The old vanilla `frontend/` is legacy. |
| `fourfour-stretch/` | Rust + C++ | **Time-stretch / pitch engine.** Safe wrapper over vendored Signalsmith Stretch (MIT, `vendor/`, built by `build.rs` at `-O2` in every profile). Used by the playback callback for tempo and key changes (`pioneer-test-ui/src/tempo.rs`, `docs/architecture/tempo-key-engine-plan.md`). Feature `count-allocs` counts C++ allocations for the callback test. |
| `app/` | Svelte 5 + TS | **The real UI** (`fourfour-app`). Browse, player strip, organize sidebar. Dev server on `:5200`. |
| `prototyping/` | Svelte | **Design-system playground** (`viewport`). Primitives → modules → screens. See `prototyping/README.md` and its `SKILL.md`. |
| `analysis/` | Python | **Analysis CLI** — `fourfour-analyze` (BPM/key/waveform/energy/cues) and `fourfour-benchmark`. Package name `fourfour-analysis`. `fourfour-analyze` defaults to `deeprhythm_essentia` (DeepRhythm BPM, Essentia key). The Lexicon port (`lexicon_port`) is benchmark/reference code, though `fourfour-benchmark run` still defaults to `--variants lexicon_port`. |
| `benchmark/` | Python | **In-repo benchmark scripts** (`scripts/cli_batch_analyze.py`), manifests, and baselines. Datasets, results, logs, and cache are local and gitignored. |
| `ui/waveform/` | JS | Waveform dev harness. `./waveform-dev <audio>` analyzes a track and opens it. |
| `mockup/`, `designs/` | HTML / Pencil | Static mockups and a design file. Not the Svelte app. |
| `docs/` | — | Planning, architecture, and reference docs. **See `docs/README.md`.** |

## Build & Run

```bash
# Rust workspace
cargo build

# Desktop app — fast dev loop (rebuilds bundle, kills old instance, relaunches)
cd pioneer-test-ui && ./dev.sh

# Frontend alone (Vite dev server on :5200)
cd app && npm run dev          # npm run check  → svelte-check

# Design-system prototyping (viewport on :5180)
cd prototyping && npm run dev

# Python analysis CLI
fourfour-analyze <files...>    # fourfour-benchmark for ground-truth runs

# Tests
cargo test --workspace         # includes usb-writer round-trip tests and the library export test
cd analysis && pytest          # Python analysis tests
```

The Rust tests cover the writer and library in software only. Format
correctness is validated against real CDJ-3000 hardware by binary bisection —
see `pioneer-usb-writer/reference-code/PIONEER.md`.

## Workspace Cargo.toml

The root `Cargo.toml` only lists the three members. It sets no `[profile.*]`
overrides, and stratum-dsp is not a dependency.

## Versioning

`pub const VERSION` in `pioneer-usb-writer/src/lib.rs`, shown in the UI toolbar.

## Architecture: the format pipeline

`pioneer-usb-writer` is **format-only** — no BPM/key/waveform detection.
Consumers fill `AnalysisResult` and hand it to the writer.

```
scanner::scan_directory()  →  Vec<Track>            (metadata via lofty)
                           ↓
              [consumer fills AnalysisResult]         (BPM, key, beat grid, waveform)
                           ↓
writer::filesystem::write_usb()                      (orchestrates all output)
  ├── copies audio to /Contents/{artist}/{file}
  ├── writer::anlz       → ANLZ0000.{DAT,EXT}        (beat grid, waveforms, cues)
  ├── writer::pdb        → export.pdb                 (legacy DeviceSQL database)
  ├── writer::onelibrary → exportLibrary.db           (OneLibrary SQLCipher database)
  └── artwork JPGs       → /PIONEER/Artwork/          (80x80 + 240x240 thumbnails)
```

**Key modules** (`pioneer-usb-writer/src`):

- `models.rs` — shared types: `Track`, `AnalysisResult` (`BeatGrid`, `WaveformPreview`, BPM, key, `CuePoint`s), `Playlist`, `ExistingTrack`/`ExistingUsbState`.
- `scanner.rs` — lofty tag reads; builds sanitized USB-relative paths.
- `writer/filesystem.rs` — orchestration; fresh write + incremental sync.
- `writer/sync.rs` — diff engine (Add/Update/Replace/Skip/Remove vs existing USB).
- `writer/pdb.rs` (~1100) — legacy DeviceSQL `export.pdb`. 20 table types, multi-page, binary string encoding.
- `writer/anlz.rs` (~525) — ANLZ `.DAT`/`.EXT`: beat grids, waveforms, color waveforms, cues, VBR sections.
- `writer/onelibrary.rs` (~900) — OneLibrary `exportLibrary.db` (SQLCipher). 22 tables.
- `reader/usb.rs` — reads back existing USB OneLibrary state (`read_usb_state()`).
- `reader/masterdb.rs` — reads Rekordbox `master.db` (SQLCipher). Tracks, cues, playlists, artwork.

For the UI side (`app/`), read `docs/architecture/ui-architecture.md` and
`docs/ui/`.

## Using the library

```rust
use pioneer_usb_writer::{reader, scanner, writer, models};

let tracks = scanner::scan_directory(Path::new("/path/to/music"))?;
let analyses: Vec<models::AnalysisResult> = tracks.iter()
    .map(|t| your_analyzer::analyze(&t.source_path))   // YOUR analyzer
    .collect();
let playlists = vec![models::Playlist { id: 1, name: "Set".into(), track_ids: vec![1,2,3] }];
writer::filesystem::write_usb(Path::new("/Volumes/USB"), &tracks, &analyses, &playlists)?;
let state = reader::read_usb_state(Path::new("/Volumes/USB"))?;   // optional read-back
```

`AnalysisResult` to populate:

```rust
AnalysisResult {
    beat_grid: BeatGrid { beats: Vec<Beat> },          // bar_position 1-4, time_ms, tempo (BPM*100)
    waveform: WaveformPreview { data: [u8; 400] },      // 5-bit height + 3-bit whiteness per byte
    bpm: f64,
    key: String,                                        // DJ notation: "1A", "5B", …
    cue_points: Vec<CuePoint>,
}
```

## Pioneer format gotchas

Hard-won from hardware testing. Full notes: `pioneer-usb-writer/reference-code/PIONEER.md`.

- **Dual format**: writer emits both `export.pdb` (legacy) and `exportLibrary.db` (OneLibrary). Modern CDJs (3000X, XDJ-AZ, OPUS-QUAD) prefer OneLibrary; older hardware uses PDB.
- **OneLibrary encryption**: SQLCipher key `r8gddnr4k847830ar6cqzbkk0el6qytmb3trbbx805jm74vez64i5o8fnrqryqls` (Pioneer's standard key — obfuscation, not security). Checkpoint WAL after writing.
- **ANLZ path hash**: CDJ derives the USBANLZ dir from the USB-relative audio path via a specific hash. Wrong → CDJ silently regenerates ANLZ files (ANLZ0001.DAT appears next to yours — diff to debug).
- **History tables must be non-empty** even for fresh exports — three reference blobs via `include_bytes!()`.
- **PDB page 0 sequence** must exceed all data page sequences or the CDJ ignores the DB.
- **Columns table (0x10)** uses a different page header — `unknown5` = num_rows, not `0x0001`.
- **DeviceSQL strings**: length-prefixed, byte markers (`0x40` long, `0x90` UTF-16LE). See `encode_string()` in `pdb.rs`.
- **PPTH tag (ANLZ)**: null-terminated UTF-16BE; `len_path` includes the null terminator bytes.
- **Color waveforms (PWV3/4/5/6/7)**: real 3-band data (low, mid, high; 150 columns/s) measured from the audio by `pioneer-usb-writer/src/waveform/` after analysis; `.2EX` carries PWV7/PWV6/PWVC, `.EXT` PWV3/PWV4/PWV5. The app draws the same bytes. Not yet tried on a player (see `docs/architecture/waveforms-3band-2026-10.md`).
- **Album-artist mapping**: PDB and OneLibrary use the first artist seen per album. VA/compilations collapse to one artist.

## Working in this repo

- **Branching**: `master` is the trunk. Both devs work close to it, split by area — Rust lib vs `app/`/`prototyping/` UI. Short throwaway branches only; push daily. Never push `master` without the owner's say-so.
- **UI work**: follow the specs in `docs/ui/` and `prototyping/` rather than inventing new layouts. Use the `ui-prototyping` skill for prototyping changes.
- **Conventional commits** (`feat:`, `fix:`, `docs:`, …), atomic, one concern each.

## Documentation Map

`docs/README.md` is the full index. Quick links:

- **Architecture** → `docs/architecture/` — `repo-overview.md`, `ui-architecture.md`, `tech-stack.md`
- **UI specs (current phase)** → `docs/ui/` — `vision.md`, `components.md`, `design-system.md`, `player-strip.md`, `organize-sidebar.md`, `waveform-dynamics.md`
- **Analysis pipeline (research/plans)** → `docs/analysis/` — current: `packaging-decision.md`, `benchmark-2026-10.md`, `bpm-grid-2026-10.md`, `key-detector-2026-10.md`; `pipeline-handoff.md` is historical; plus `experimentation-path.md` and Lexicon reverse-engineering
- **Pioneer format reference** → `pioneer-usb-writer/reference-code/PIONEER.md`
- **Process** → `docs/process/` — `todo.md`, `lessons.md`
- **Archive** → `docs/archive/` — superseded handoffs

## Unattended

platform: box (Rust + Svelte build on dev); audio, Tauri window, USB export and CDJ checks are `mac`
validation: `cargo build --workspace && cargo test --workspace && (cd app && npm ci && npm run check && npm run build)`
may land: Rust backend, tests, type fixes, docs, scripts
must ask: user-visible (UI, copy, UX, visuals), USB format output, library DB schema, roadmap/scope
never: push master, release, notarise, sign, publish crates/packages, secrets, delete benchmark data
scope fence: no new deps without a BLOCKED report; no edits to `mockup/`, `designs/`, `prototyping/` unless the entry says so
lane rules: none; format correctness is proven on CDJ-3000 hardware only (see Pioneer format gotchas)
lanes: `pioneer-usb-writer/src/` and USB/hardware behaviour belong to Merlin's brother (morizkraemer); workers add tests and report findings there, never change format code
ui reviews: judge on intuitive, efficient workflows on their own terms, never on Rekordbox familiarity (beating Rekordbox UX is the point)
polish vs rebuild: when a task improves something that already works, polish the existing code in place; replacing it (new renderer, new pipeline) is a BLOCKED question to the orchestrator
autonomy: tight (draft; no queue run yet) — everything stays on branches; only state, docs and benchmark/analysis reports land
own priorities: finish paused work → bugs → tests → perf → cleanup, inside Merlin's lane (app UX code on branches, analysis prototype, tests); experiments: max 1 per run, analysis prototype only (prototyping/analysis-packaging), never landed
targets: analysis in Rust: beat-grid phase/drift (grid within ±20 ms of Rekordbox: 47% now) and key ≥60% exact on Beatport (57% now); sync UI per docs/ui/usb-sync-decisions.md, built only after Merlin settles its open questions
ask before queue: anything in the brother's lane (usb-writer src, sync engine, format, hardware); building or changing sync UI while its open questions stand; copying more of Merlin's music or library data to dev; the 2026-10-30 → 2026-11-16 holiday (nothing user-visible lands); any push
run limits: ≤2 workers; ≤4 analysis processes; one `cargo build --release` at a time; spawn nothing while `free -m` available < 1500 MB (earlyoom killed previews on 2026-10-15); datasets under benchmark/datasets are read-only; results/scratch in benchmark/results (gitignored)

## Orchestrator

This project runs as one long-lived orchestrator thread (see the `orchestrator` skill), on the `dev` box (Proxmox VM on `lab`) as a Claude Desktop SSH session in this folder. Live state: `docs/STATE.md`; history: `docs/journal.md`; rulebook: `## Unattended` above.

- Workers are background subagents in worktrees; capped `claude -p` credit runs only for `[drain]` entries.
- Hard gates are the `permissions.ask` rules in `.claude/settings.json`; only the orchestrator runs them, in the foreground. Everything else Merlin decides goes to STATE.md "Waiting on Merlin" with a recommendation and a branch.
- Merlin keeps roadmap, anything user-visible, outward actions and taste.
