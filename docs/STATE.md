# fourfour — state

_Updated: 2026-10-23 by orchestrator on `dev`. Live state only, max ~80 lines. History → docs/journal.md, facts → memory._

## Direction
Not in Compass (baseline 2026-09-24: creative block to 30 Oct, then Pad, then Bounce). Quiet engineering only; never takes Merlin's focus hours. Holiday 2026-10-30 → 2026-11-16: nothing user-visible lands.
Roles: Merlin owns app UX (interface, player, library, analysis quality) and cross-platform testing; his brother (GitHub morizkraemer) owns the USB format, sync engine and hardware tests. Merlin travels: no hardware.
Analysis: Rust-native (Rust DSP + Beat This! ONNX), permissive licences only, no Python/AGPL. Key bar ≥60% exact on Beatport.
USB sync: only chosen playlists go on a stick; plug in = sync; ask only on removals / won't fit / unanalyzed; never delete files fourfour didn't write. Design log: docs/ui/usb-sync-decisions.md; engine spec for the brother: docs/ui/usb-sync-spec.md.

## Ship target
Public macOS beta: a .dmg that scans a folder, analyzes it in Rust and exports a USB that plays on a CDJ-3000. Current queue done when: Rust analysis meets the grid/key bars, the sync UI round 3 is built on a branch and reviewed, and the brother's engine supports playlist-scoped sync.

## Rulebook
AGENTS.md ## Unattended (queue fields drafted 2026-10-23: autonomy tight). No Tauri windows, audio output or USB writes on dev; those are `[mac]`.

## Waiting on Merlin
1. Sync round 3 (https://claude.ai/artifact/1cCJTk3r92jC6kmJmz1nnJ): panel closed → whole stick view as drop target, or a "Put on stick" strip? — rec: strip — branch: none
2. Copy a playlist to another stick: Keep updated on by default? — rec: yes — branch: none
3. Build round 3 into the app (replaces ui-sync-flow's first attempt, keeps the stick header)? — rec: yes after 1–2 — branch: ui-sync-flow
4. Key detector weights fit on CC BY-SA 4.0 data — rec: ship only weights trained on permissively licensed data — branch: none
5. Old stash@{0} (scrub-pause waveform tweaks) — rec: build on a branch for review — branch: none
6. `.claude/settings.local.json` allows `git push *`, which bypasses the push ask rule — rec: remove that allow line (Merlin edits it) — branch: none
8. [accepted 2026-10-23 per rec] Add vitest as an app dev dependency for unit tests (Q18, branch only) — rec: keep — branch: q18-app-tests
7. Push master (30 commits ahead, docs + analysis prototype + tests) — rec: push — branch: master

### Needs Merlin's ears/eyes (laptop)
- Preview, master: https://dev.tail87069a.ts.net:4200 (`preview start fourfour app-review`); dies on OOM, restart by hand.
- Playback ear check: play on select, scrub/seek while playing, an MP3 with a damaged frame.

### Brother's lane
- Sync engine changes per docs/ui/usb-sync-spec.md (playlist scope, ownership manifest, dry-run plan, progress events, capacity, removable-drive filter, wipe guard).
- `get_mounted_volumes` lists the system disk ("Macintosh HD") and offers Wipe: must fix.
- Q6 key source PDB vs OneLibrary; .2EX / PWV6-7 placement for CDJ-3000X; CDJ-3000 round-trip of a current export.

## Queue
### Q1–Q4, Q7, Q9–Q13 [landed] — playback, svelte-check 0, export tests, browser preview, benchmark, cache fix, packaging decision, key detector (57%), BPM/grid (BPM ±0.1 86%, grid ±20 ms 47%). See journal + docs/analysis/.
### Q5 Unsigned macOS build [queued] [mac] — `cargo tauri build` via mac-queue; report artifact + log; no signing. 30 min
### Q6 Key source PDB vs OneLibrary [handed to brother]
### Q8 Cross-platform CI [queued] [box] — Actions on macOS/Windows/Linux (build, test, check, unsigned tauri build). Branch only; first run on the shared repo needs Merlin's OK. 45 min
### Q14 USB sync UI [waiting on Merlin] — first attempt on ui-sync-flow (b510e70, preview :4240 when started); round 3 mockups decide the rebuild.
### Q15 Grid phase + drift [done, branch q15-grid 8b9a3d0; doc landed] — grid ±20 ms 47%→72% (148/205), BPM ±0.1 86%→88%; full-track Beat This! + tempo comb + integer snap + mp3 encoder-delay fix; ~25 s/track. Code stays on branch (tight); mp3-delay rule needs a hardware check (brother).
### Q16 Key detector round 2 [done, branch q16-key a6b4929; doc landed] — still 57.3% exact (70.9% incl. relative+fifth), n=199; augmentation/MLP/segments within noise. Next: check Beatport label noise, more permissive training data, a parallel-key feature. Weights gitignored (CC BY-SA, measurement only).
### Q17 Doc drift cleanup [landed ac933c8] — own priority: AGENTS.md test-suite + stratum/src/analyzer claims, pioneer-test-ui README, pipeline-handoff status, docs/README index for the new analysis/ui docs.

### Q18 App unit tests [done, branch q18-app-tests 4dba72d, rebased on master] — 41 vitest tests (analysis validators, mock, library helpers, menus), check + build green; npm audit advisories not reviewed; untested: mapTrackInfoToLocal, normalizeTrackInput, sortTracks (need export/extraction)

### Q19 Clone research (item 7) [running] — report in scratchpad/clones/report.md
### Q20 Startup + UI speed (items 1, 11) [running] — branch q20-speed

## Run
status: running · since 2026-10-23 14:07 · cap 2 workers · autonomy tight · overnight, until Merlin says stop
targets (Merlin, direct): 1 fastest startup · 2 bulk import + analysis speed · 3 sync several USBs at once · 4 instant track load incl. waveform · 5 better beat grid · 6 Rekordbox library import works · 7 research other Rekordbox clones (incl. new "lite" one), reuse code if licence allows · 8 review the analysis pipeline twice for speed + quality · 10 audio engine: no crashes, fast, responsive · 11 UI lightning fast
plan: Q19 clone research (7) ∥ Q20 startup + UI speed (1, 11) → Q21 analysis review + bulk speed + grid (8, 2, 5) → Q22 audio engine (10) → Q23 track/waveform load (4) → Q24 Rekordbox import (6) → Q25 multi-USB sync (3) → second analysis review pass (8) → own priorities
rules: all code on branches (tight); docs/reports land. Items 3/6 touch the brother's lane (sync engine, master.db reader): branch only, flagged for him. Third-party code: licence-checked, inspected in an isolated dir, never executed blindly.

## Rundown
Since 10-15 nothing ran. Master holds the Rust analysis prototype (BPM/grid/key), benchmark docs, UI review and three sync mockup rounds. Open: Merlin's sync round 3 answers, brother's engine work, Q15/Q16 proposed.
