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
2. Benchmark data (2.9 GB datasets, 1 GB results/cache, analysis .venv) stayed on the Mac — rec: copy datasets to dev for the analysis benchmark — branch: none
3. Old stash@{0}: scrub-pause waveform changes (UI) — rec: build it on a branch for your review — branch: none

### Needs Merlin's ears/eyes (laptop)
- Playback ear check (Q1 landed): play on select, scrub/seek while playing, an MP3 with a damaged frame.

### Brother's lane (hardware)
- CDJ-3000 round-trip of a current master export (last proven before the Svelte app).
- Q6 key-source fix and the .2EX/colour-waveform layout (research synthesis) are format changes for him.

## Queue

### Q1 Fix player playback (no audio) [landed] [box] [drain]
goal: root-cause why the Svelte player produces no audio (docs/process/todo.md 2026-06-04) and fix it in the Rust/Tauri path.
acceptance: root cause named; a cargo test proves the playback path yields non-silent decoded samples; stash@{0} (scrub-pause changes) reviewed and used or noted.
validation: AGENTS.md validation + `cargo test -p pioneer-test-ui`
may write: pioneer-test-ui/src/, app/src/stores/player.svelte.ts, app/src/services/playback.svelte.ts, docs/process/todo.md
do not touch: USB format code, UI layout/visuals
timebox: 60 min

### Q2 svelte-check to zero errors [landed] [box] [drain]
goal: clear the 61 svelte-check errors without changing behavior or visuals.
acceptance: `cd app && npm run check` exits 0; no rendered change.
validation: `cd app && npm run check && npm run build`
may write: app/src/ except the two Q1 player files, app/tsconfig.json
do not touch: prototyping/ (exclude it from the check if it leaks in), styles
timebox: 45 min

### Q3 Export round-trip tests [landed] [box] [drain]
goal: first Rust tests for pioneer-usb-writer: write a fixture library to a temp dir, parse PDB/ANLZ back, assert tracks, playlists, cues, artwork.
acceptance: `cargo test -p pioneer-usb-writer` runs ≥5 meaningful tests, green; fixtures are small and committed.
validation: `cargo test --workspace`
may write: pioneer-usb-writer/tests/, pioneer-usb-writer/Cargo.toml (dev-deps only after BLOCKED)
do not touch: format-writing code (report bugs found as BLOCKED)
timebox: 60 min

### Q4 Library export smoke test [landed] [box] [drain]
goal: pioneer-library test: import a fixture folder into a temp SQLite library, export to a temp "USB" dir, re-sync incrementally.
acceptance: test covers full export + one incremental sync, green.
validation: `cargo test --workspace`
may write: pioneer-library/tests/
do not touch: schema, format code
timebox: 45 min

### Q5 Unsigned macOS build [queued] [mac]
goal: `cargo tauri build` via mac-queue (headless build clone) produces an unsigned .app/.dmg; record size, build time, failures.
acceptance: artifact path + build log tail in the journal.
validation: the build itself
may write: none (report only)
do not touch: signing, notarisation, release
timebox: 30 min

### Q6 One key source for PDB and OneLibrary [queued] [box]
goal: OneLibrary writes the key from analysis.key, the PDB from track.key; an empty analysis.key gives an empty OneLibrary key and makes sync see the track as changed. Use one source for both.
acceptance: a round-trip test with empty analysis.key shows the same key in PDB and OneLibrary and sync skips the unchanged track.
validation: AGENTS.md validation
may write: pioneer-usb-writer/src/ (key path only), pioneer-usb-writer/tests/
do not touch: other format fields
timebox: 30 min
note: changes USB output → lands only after Merlin's OK (CDJ check)

## Rundown
Landed: Q2 (svelte-check 61 → 0), Q1 (decode skips bad packets, play waits for stop, 4 decode tests). Master pushed to GitHub; only branch left is master.
Not heard yet: playback on the Mac.
Landed: Q3 (15 usb-writer round-trip tests), Q4 (library export + sync test). All 47 tests green, no format bugs. Q6 queued (key mismatch). Next: Q5 macOS build [mac].
