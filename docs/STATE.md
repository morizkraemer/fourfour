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

18. [accepted 2026-10-29 per rec] fb-dupes adds Rust deps `tar` + `filetime` (MIT/Apache) to pioneer-library for the backup .tar.gz — rec: keep — branch: ov-1028

19. Waveform detail cache cap is 512 MiB (~100 six-minute tracks at 5 MB each) - rec: raise to 2 GiB - branch: none (q50-trackload follow-up)

20. Q50: analysis stores the waveform detail from its own decode, but MP3 differs from measure_detail (symphonia 0.6 vs 0.5) - rec: hold until the app moves to symphonia 0.6 - branch: q50-analysis2 (fe78598, not merged)
21. Q50: rten 0.27 bump - logits differ up to 8.6e-5, slower - rec: drop - branch: q50-rten27
22. [accepted 2026-10-30 per rec] Q50: 8 GB Macs run 3 beat-model chunks at once (~2 GB peak, about half the model throughput) - rec: keep - branch: ov-1028

23. [accepted 2026-10-30 per rec] Q50 import: additive index idx_artwork_track (created on open, no version bump; cold track list 1.3 s → 0.1 s at 10k covers) and import commit with synchronous=NORMAL + background checkpoint (power loss right after an import can roll it back; DB stays consistent) - rec: keep - branch: ov-1028

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

### Q50 Speed: lightning-fast startup, track load, import, analysis [running] [box] — Merlin 2026-10-29: "hyper optimize for speed … this needs to be lightning fast". Goal: measured budgets per path, then fix the worst first. Paths: cold/warm startup to interactive (10k-track library), track load to first audible + full waveform, import of a 1k-file folder (scan → plan → commit → UI), analysis throughput (tracks/min, per-stage profile: decode, resample, mel, ONNX, fit, key, bands, writes), UI responsiveness during all of them. Acceptance: a perf report with before/after per path, a repeatable perf suite (extend app/perf + smoke perf10k + a Rust bench), every win landed on a branch with green checks; BPM/grid/key results unchanged. Fleet split: one worker per path + one for the shared perf harness first; one writer per path set. Builds on fb-wave/fb-bulk findings (docs/analysis/grid-offset-2026-10-29.md, perf notes in journal 2026-10-29). Timebox: one overnight run. Run conditions (Merlin 2026-10-29): seat only, no API/credit runs; visible app runs on the Mac (mac-queue --visible) allowed only after 00:00 Mac local time (WITA, UTC+8); Mac on AC from bedtime. Merlin runs a Grokbot watcher that pings this thread when the seat limit resets → on that ping, resume every stopped worker (SendMessage), check limits first.

## Run
status: proposed · since 2026-10-30 13:30 WITA · two weeks to 2026-11-16 · cap 2 · seat only (5 h windows, resume after resets) · autonomy: green work lands on ov-1028 + Mac install; never master, never push
targets (Merlin Q&A 2026-10-30): beta-ready on ov-1028, and as fast as possible at 10k+ tracks (startup, analysing 10k+, track load, scroll). Order: speed first, then beta. "nothing's off limits": every item below may be built; hard gates and the brother's format bytes still hold
plan:
0. merge q50-report summary (c3476bb) into ov-1028
Week 1, speed (measured on a real-size 10k+ fixture, before/after each):
1. 10k+ fixture + perf budgets in scripts/perf.sh; Mac timing script for the Mac-night batch
2. startup to interactive at 10k-50k (cached last list, lazy work after first paint)
3. analysis at 10k+: throughput (decode once, ONNX batching/threads), steady memory, resumable queue across restarts
4. track load to first audio + full waveform; detail cache cap 2 GiB (Waiting 19)
5. UI during analysis (Merlin 2026-10-30: still laggy on the Mac, analysis at ~75% CPU): leave UI headroom on the Mac (fewer analysis threads or a pause while scrolling/playing), measure in the Mac-night batch; remaining pulse cost, library mutex vs get_analysis_data, WKWebView-specific fixes after Mac numbers
6. import of 10k+ files end to end
Week 2, beta:
7. analysis bars: key >=60% exact (now 58.3%), grid; bar 1 rule (Waiting 16) by data, listening check queued for Merlin
8. key templates refit on permissively licensed labels (Waiting 4), on a branch, licence note for Merlin
9. unsigned .dmg build (Q5) via mac-queue + install check
10. sync UI round 3 on recs (Waiting 1-3: strip, keep-updated on, replace ui-sync-flow attempt)
11. CI workflow (Q8) on branch ov-ci, no push (first run needs Merlin's push OK)
12. bug sweep from smoke + journal followups; then own priorities (rulebook order)
review: https://dev.tail87069a.ts.net:4220 (phone review cards: lab/, add with lab/scripts/review-shot.ts + hub:card, read answers with hub:notes) · app preview https://dev.tail87069a.ts.net:4210 (ov-1028, no Rust backend; restart: preview start fourfour app-review --dir <q41-int>/app --port 4210)
each finished user-visible item gets a review card; when emberland says @lab/kit (lab-infra packages/lab-kit) landed, swap lab/ inbox UI to mountInbox (keep cards/media/capture script; no name-based logo, working title); Grok gets the 4220 link
Mac nights: one visible batch per week (### Visible Mac batch): 10k startup/scroll/analysis timings, .dmg install; Grok told once per batch
Hard gates: no push, no master merge, no deploy, no CDJ/USB writes, brother's format/engine lane read-only

## Rundown
Overnight 2026-10-28 (ov-1028 232ca15, branch only, not on master): round 12–13 bug review (3 fixes), grid anchor persisted (×2/÷2 exact), new big-player layout A (pencil BPM/key, memory cue chips, no edit mode), Playwright smoke harness (13 flows), 10k perf pass (import 15.6 s → 0.5 s, save ~300 → 20 ms), Rust analyzer status (grid ±20 ms 80.5%, key 58.3%), truncated audio survives, weights bundled. CI on separate branch ov-ci (needs push OK).
Checks: 700 vitest, smoke 13/13, cargo all ok, cut-wav BPM/grid with real weights ok. One cargo test failed once under load (unidentified).
Next: install ov-1028 on the Mac (offline overnight) → Merlin tests → merge to master; decisions 15–17 + bar 1 (16).
