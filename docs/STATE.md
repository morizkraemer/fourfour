# fourfour — state

_Updated: 2026-10-15 06:05 by orchestrator. Live state only, max ~80 lines. History → docs/journal.md, facts → memory._

## Direction
Not in Compass (baseline 2026-09-24: creative block to 30 Oct, then Pad, then Bounce). Thread drains quietly on engineering work; it never takes Merlin's focus hours. Holiday 2026-10-30 → 2026-11-16: nothing user-visible lands.

Roles: Merlin owns the app UX (interface, player, library, analysis quality: grid/BPM/key) and cross-platform testing; his brother (GitHub morizkraemer) owns the USB format and hardware testing. Merlin is travelling: no hardware.

## Ship target
Public macOS beta: a downloadable .dmg on GitHub that scans a music folder, analyzes it and exports a USB that plays on a CDJ-3000. Done for the current queue: a clean master that builds, type-checks, plays audio, and has round-trip tests for the export, so the beta build is the only open step.

## Rulebook
AGENTS.md ## Unattended. Thread-specific limits: no worker runs Tauri windows, audio output or USB writes on dev; those are `[mac]`.

## Waiting on Merlin
1. Does this thread touch the brother's lane (usb-writer, sync, format)? — rec: no, only tests + reports; format changes go to him — branch: none
2. Copy rekordbox-testdata audio (652 tracks of your music on the Mac) to dev for Q9 — rec: yes — branch: none
3. Old stash@{0}: scrub-pause waveform changes (UI) — rec: build it on a branch for your review — branch: none

### Needs Merlin's ears/eyes (laptop)
- Playback ear check (Q1 landed): play on select, scrub/seek while playing, an MP3 with a damaged frame.

### Brother's lane (hardware)
- CDJ-3000 round-trip of a current master export (last proven before the Svelte app).
- Q6 key-source fix and the .2EX/colour-waveform layout (research synthesis) are format changes for him.

## Queue

### Q1–Q4 [landed]: playback fix, svelte-check 0, usb-writer round-trip tests, library export test (see journal)

### Q5 Unsigned macOS build [queued] [mac]: `cargo tauri build` via mac-queue, report artifact + log; no signing. 30 min

### Q6 One key source for PDB and OneLibrary [queued] [box] — brother's lane: format change, hand to him

### Q7 Browser preview with sample data [queued] [box]
goal: Merlin can open the app on laptop/phone while travelling: `preview start fourfour app-review` serves the browser build with the existing mock data (services/tauri.svelte.ts); every main screen (browse, detail, player, organize, settings) renders with sample tracks instead of erroring on missing Tauri.
acceptance: preview URL loads; headless screenshots of each main screen show sample data and no console errors; mock gaps fixed in the mock layer only.
validation: AGENTS.md validation + headless check of the preview URL
may write: app/src/services/tauri.svelte.ts, app/src/harness.ts, new app/src/mocks/ if needed
do not touch: real UI components, styles, Rust
timebox: 45 min

### Q8 Cross-platform CI [queued] [box]
goal: GitHub Actions on macOS, Windows, Linux: cargo build + test, app check + build, and an unsigned `tauri build` per OS as an artifact.
acceptance: workflow file on a branch; runs green on a fork-free test (act or a branch push after Merlin's OK); per-OS failures listed.
validation: workflow lint (actionlint) + local cargo/npm validation
may write: .github/workflows/
do not touch: signing, secrets, release jobs
timebox: 45 min
note: a branch push starts CI on the shared repo → push only after Merlin's OK

### Q9 Analysis accuracy benchmark [queued] [box]
goal: numbers for the current backend (deeprhythm_essentia): key accuracy on beatport-edm-key (Beatport labels); BPM + key agreement with Rekordbox on rekordbox-testdata-2025 (652 tracks, Merlin's library, ground truth from master.db); state that beat-grid ground truth is missing and propose how to get it.
acceptance: docs/analysis/benchmark-2026-10.md with per-signal tables (exact, octave-tolerant, Camelot-neighbour), the denominators, failures, runtime per track.
validation: the benchmark run itself; `cd analysis && pytest -q`
may write: docs/analysis/, benchmark/manifests/ (paths rewritten for dev), benchmark/results/ (ignored)
do not touch: analysis backends (measure, don't tune)
timebox: 90 min
needs: datasets on dev (beatport copying now); rekordbox-testdata audio is Merlin's music on the Mac → his OK to copy

### Q10 Analysis packaging decision [queued] [box]
goal: a decision doc for shipping analysis without a dev venv: (a) bundled Python (PyInstaller/uv-built runtime), (b) Rust-native (stratum-dsp, Beat This! ONNX, a key detector), (c) hybrid. Per option: size, cold start, per-track speed, accuracy vs Q9, licence (Essentia/DeepRhythm are AGPL), cross-platform effort. Prototype the cheapest credible option on 3 fixture tracks.
acceptance: docs/analysis/packaging-decision.md with a recommendation and the prototype's numbers; nothing wired into the app.
validation: prototype runs on dev without analysis/.venv
may write: docs/analysis/, prototyping branch only
do not touch: app, Tauri, usb-writer
timebox: 90 min
note: the choice is Merlin's; run after Q9

## Rundown
Landed: Q2 (svelte-check 61 → 0), Q1 (decode skips bad packets, play waits for stop, 4 decode tests). Master pushed to GitHub; only branch left is master.
Not heard yet: playback on the Mac.
Landed: Q3 (15 usb-writer round-trip tests), Q4 (library export + sync test). All 47 tests green, no format bugs. Q6 queued (key mismatch). Next: Q5 macOS build [mac].
