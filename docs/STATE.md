# fourfour — state

_Updated: 2026-10-15 06:05 by orchestrator. Live state only, max ~80 lines. History → docs/journal.md, facts → memory._

## Direction
Not in Compass (baseline 2026-09-24: creative block to 30 Oct, then Pad, then Bounce). Thread drains quietly on engineering work; it never takes Merlin's focus hours. Holiday 2026-10-30 → 2026-11-16: nothing user-visible lands.

## Ship target
Public macOS beta: a downloadable .dmg on GitHub that scans a music folder, analyzes it and exports a USB that plays on a CDJ-3000. Done for the current queue: a clean master that builds, type-checks, plays audio, and has round-trip tests for the export, so the beta build is the only open step.

## Rulebook
AGENTS.md ## Unattended. Thread-specific limits: no worker runs Tauri windows, audio output or USB writes on dev; those are `[mac]`.

## Waiting on Merlin
1. Beta v0.1 scope — rec: scan → analyze → export only; player and organize ship later — branch: none
2. Push master (7 ahead of origin, no CI, nothing deploys) — rec: push — branch: master
4. Benchmark data (2.9 GB datasets, 1 GB results/cache, analysis .venv) stayed on the Mac — rec: leave until analysis work resumes — branch: none

3. Old stash@{0}: scrub-pause waveform changes (UI) — rec: queue as its own entry after your ear check — branch: none

### Needs you at the Mac
- CDJ-3000 round-trip of a current master export (last proven before the Svelte app).
- Ear check of playback (Q1 landed): play on select, scrub/seek while playing, an MP3 with a damaged frame. If still silent, look for "audio playback unavailable" on stderr.

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

### Q3 Export round-trip tests [queued] [box] [drain]
goal: first Rust tests for pioneer-usb-writer: write a fixture library to a temp dir, parse PDB/ANLZ back, assert tracks, playlists, cues, artwork.
acceptance: `cargo test -p pioneer-usb-writer` runs ≥5 meaningful tests, green; fixtures are small and committed.
validation: `cargo test --workspace`
may write: pioneer-usb-writer/tests/, pioneer-usb-writer/Cargo.toml (dev-deps only after BLOCKED)
do not touch: format-writing code (report bugs found as BLOCKED)
timebox: 60 min

### Q4 Library export smoke test [queued] [box] [drain]
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

## Rundown
Landed: Q2 (svelte-check 61 → 0), Q1 (decode skips bad packets, play waits for stop, 4 decode tests). Master ab88a4c+, all checks green, not pushed.
Not heard yet: playback on the Mac.
Next: Q3 export round-trip tests, Q4 library export test [box]; Q5 macOS build [mac].
