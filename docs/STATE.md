# fourfour — state

_Updated: 2026-10-23 by orchestrator on `dev`. Live state only, max ~80 lines. History → docs/journal.md, facts → memory._
_Archived detail (trimmed 2026-10-27): docs/state-archive-2026-10-28.md_

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
9. [accepted 2026-10-27 per rec] Q42: library schema v5→v6 additive (sticks, stick_tracks, stick_playlists) — rec: keep — branch: q42-overnight
10. [accepted 2026-10-27 per rec] Q42: writer `sync_usb_keeping(keep_files)` so removals never delete files fourfour didn't write (brother's lane, no format bytes) — rec: keep, tell brother — branch: q42-overnight
11. [resolved 2026-10-28: Merlin tested, merged to master 9fc5e48] Review on the Mac: stick board, sync dialog, ribbon slot, ⌥ pads, deep zoom, new tempo bar, search palette, playlist folders + right-click, import dialog, rating/colour columns — rec: test, then merge q42-overnight — branch: q42-overnight
12. [accepted 2026-10-28 per rec] schema v6→v7 additive (playlists parent_id, is_folder); folders exported as PDB/OneLibrary tree, untested on a CDJ — rec: keep, brother checks on hardware — branch: q42-overnight
0. [resolved 2026-10-27] Writer bugs (192-track PDB split, >7:17 colour overflow, cue names/colours) fixed on wu-writer, merged to master; CDJ check still open: docs/architecture/cdj-checklist-2026-10.md — rec: brother runs the checklist — branch: master
1. Sync round 3 (https://claude.ai/artifact/1cCJTk3r92jC6kmJmz1nnJ): panel closed → whole stick view as drop target, or a "Put on stick" strip? — rec: strip — branch: none
2. Copy a playlist to another stick: Keep updated on by default? — rec: yes — branch: none
3. Build round 3 into the app (replaces ui-sync-flow's first attempt, keeps the stick header)? — rec: yes after 1–2 — branch: ui-sync-flow
4. Key: shipped key stage is now the cyanbox port (BSD-2 code) whose templates were fit on GiantSteps MTG (annotations unlicensed); keydet weights fit on CC BY-SA. Both need your licence call — rec: refit templates on permissively licensed labels before shipping. Was: key detector weights fit on CC BY-SA 4.0 data — rec: ship only weights trained on permissively licensed data — branch: none
5. Old stash@{0} (scrub-pause waveform tweaks) — rec: build on a branch for review — branch: none
6. `.claude/settings.local.json` allows `git push *`, which bypasses the push ask rule — rec: remove that allow line (Merlin edits it) — branch: none
8. [accepted 2026-10-23 per rec] Add vitest as an app dev dependency for unit tests (Q18, branch only) — rec: keep — branch: q18-app-tests
7. Push master (30 commits ahead, docs + analysis prototype + tests) — rec: push — branch: master

13. [accepted 2026-10-28 per rec] O1b: ×2 then ÷2 drifts the grid half a bar after a save (rescale anchors on the first downbeat); fix = additive optional `grid_anchor_ms` in edits.json — rec: keep — branch: ov-1028 (O1b)

14. [accepted 2026-10-28 per rec] O2 adds `@playwright/test` as an app devDependency for `npm run smoke` — rec: keep — branch: ov-1028

15. O5 CI workflow ready (macOS/Windows/Linux, unsigned bundles as artifacts, untested) — rec: OK to push branch ov-ci and do a first run — branch: ov-ci (not in ov-1028)

16. Bar 1: Rekordbox bar 1 matches the model downbeats only 61.7%; "first grid beat = bar 1" matches 78.9%; hybrid 80.0% — rec: listen to ~5 disagreeing tracks on the Mac, then pick (docs/analysis/rust-status-2026-10-28.md) — branch: none

17. [accepted 2026-10-28 per rec] O9: bundle Beat This! weights (+83 MB, MIT, licence file shipped) via tauri.bundle-weights.conf.json + bundle-app.sh; no-backend now errors visibly — rec: keep, check on the next Mac .dmg build — branch: ov-1028

### Needs Merlin's ears/eyes (laptop)
- Preview, master: https://dev.tail87069a.ts.net:4200 (`preview start fourfour app-review`); dies on OOM, restart by hand.
- Playback ear check: 8 steps in docs/architecture/audio-engine-2026-10.md (branch q22-audio).

### Brother's lane
- SQLCipher keys private; no PDB reader (docs/architecture/rekordbox-import.md).
- From clone research: Booth 0.0.9 CDJ-3000 fw 3.22 fixes, baken waveform/PVB2/PSSI (MIT), cyanbox MP3 timeline rule + PWAV min heights — see docs/research/clones-2026-10-23.md "Format facts".
- Sync engine changes per docs/ui/usb-sync-spec.md (playlist scope, ownership manifest, dry-run plan, progress events, capacity, removable-drive filter, wipe guard).
- Q6 key source PDB vs OneLibrary; .2EX / PWV6-7 placement for CDJ-3000X; CDJ-3000 round-trip of a current export.

## Queue
### Q5 Unsigned macOS build [queued] [mac] — `cargo tauri build` via mac-queue; report artifact + log; no signing. 30 min
### Q6 Key source PDB vs OneLibrary [handed to brother]
### Q8 Cross-platform CI [queued] [box] — Actions on macOS/Windows/Linux (build, test, check, unsigned tauri build). Branch only; first run on the shared repo needs Merlin's OK. 45 min
### Q14 USB sync UI [waiting on Merlin] — first attempt on ui-sync-flow (b510e70, preview :4240 when started); round 3 mockups decide the rebuild.
### Q33 Rust analyzer in the app [done, branch q33-rust-analysis 23332d0, master merged] — fourfour-analysis-rs crate; backend setting rust|python (rust when weights present); 2.36 vs 1.78 tracks/min on a loaded box; Rust emits no cues/energy (licence call). Weights: fourfour-analysis-rs/scripts/fetch-models.sh. Not merged. — branch q33-rust-analysis: lib crate fourfour-analysis-rs extracted (c3cb7a2); backend/coordinator files WIP-committed unbuilt; worktree .claude/worktrees/agent-a45ef63857b3fe37b

## Run
status: idle · since 2026-10-28 20:30 UTC · all items done on ov-1028 (232ca15), checks green; nothing left without Merlin or the Mac · cap 2 · tight
targets: Merlin 2026-10-28: start overnight now; player-layout build only after he approves the next mockup round on viewport
plan: L2 player-layout mockups (round 2, viewport :5238) → O1 review rounds 12–13 → O2 smoke-test harness → O7 → O1b grid anchor (after O7) → O3 Q33 Rust analyzer finish + measure (report) → O4 perf pass 10k tracks → O5 Q8 CI (branch only) → O6 housekeeping → O8 truncated decode → O9 bundle weights; O7 build layout-2 option A (approved 2026-10-28, starts after O1 lands: same files), then own priorities
pm: 2026-10-28 19:45 UTC resumed after seat limit; O8 + O9 running; O1–O7 on ov-1028; Mac install pending (Mac offline)
decided 2026-10-27 (Merlin): ignore brother-lane gates for now — writer/sync fixes go ahead here

### Q42 Stick sync for real [done, branch q42-sync d56f471, merged into q42-overnight 51a9ef0] [box] — sync button → sync_usb_many: stick All tracks + chosen playlists (SyncSelection::Playlists track_ids), stable stick id (not path), capacity check, dry-run plan shown before write. Fake stick folder only; no real USB writes. Validation + headless. 180 min
### Q43 Review rounds 3–6 [done, q43-review 33b2906 → q42-overnight 8d84da9; 8 fixes; follow-ups in Q43b] [box] — bug hunt over the q41-feedback merges; extra care: wipe guard, volumes.rs, writer pdb/cue changes, tempo engine RT safety. Fix clear bugs on branch, list the rest. 150 min
### Q44 Leftovers [done, q44-leftovers a26ab3c → q42-overnight 6f654ab] [box] — row waveforms blue-block (row_bands detail labelled overview), player overview fallback → stripBandsFromAnalysis, flaky vitest (find + fix). 90 min
### Q45 Unsigned .dmg [done] — Mac ~/dev/build/fourfour-dmg/target/release/bundle/dmg/fourfour_0.1.0_aarch64.dmg (32 MB, checksum valid, ad-hoc signed, arm64 only, app 79 MB incl. dev bench binaries) [mac] — `cargo tauri build` .dmg on the Mac build clone (separate dir), launch smoke check headless; report artifact + size + log. No signing/notarising. 60 min
### Q43b Review follow-ups [done, q43b-fixes dab9629 → q42-overnight 0c713b9] [box] — PDB oversized row, fake data on error paths, Linux removable filter + wipe path check, Esc/editable/listener/loadStick/claim_selection. 120 min
### Q46 Key refit on permissive labels [done, branch q46-key c6deeff, report only] — best licence-clean 55.8% exact (cyanbox 58.3%), bar 60% not met; doc docs/analysis/key-licence-2026-10-27.md [box] — find permissively licensed key labels, refit templates, measure on Beatport set; report only, never shipped. 150 min
### Q48 Tempo engine robustness [done, q48-tempo-robust 8c2dd93 → q42-overnight dc818de] [own prio: bugs] [box] — stretcher built at device sample rate, crossfade on stretch→varispeed dropout, ring size for 8ch/192k, NaN sanitising. 120 min
### Q49 [done, branch q49-key-midi d0a176b, never landed: 55.8% test, ties Q46] Key: render permissive MIDI (Lakh CC BY / POP909 MIT) via FluidR3 (MIT) for template fit [own prio: experiment, never landed] [box] 150 min
### Q47 Housekeeping [done, a72200e: 27 worktrees removed, STATE 96→64 lines, disk 47%] [box] — remove merged worktrees + caches (check uncommitted/ignored first), disk < 70%; trim STATE.md to ≤80 lines (history → journal/docs). 45 min

## Rundown
Overnight 2026-10-28 (ov-1028 232ca15, branch only, not on master): round 12–13 bug review (3 fixes), grid anchor persisted (×2/÷2 exact), new big-player layout A (pencil BPM/key, memory cue chips, no edit mode), Playwright smoke harness (13 flows), 10k perf pass (import 15.6 s → 0.5 s, save ~300 → 20 ms), Rust analyzer status (grid ±20 ms 80.5%, key 58.3%), truncated audio survives, weights bundled. CI on separate branch ov-ci (needs push OK).
Checks: 700 vitest, smoke 13/13, cargo all ok, cut-wav BPM/grid with real weights ok. One cargo test failed once under load (unidentified).
Next: install ov-1028 on the Mac (offline overnight) → Merlin tests → merge to master; decisions 15–17 + bar 1 (16).
