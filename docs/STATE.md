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
4. [accepted 2026-10-31 per rec, landed on ov-1028 3e5ec61; clean 56.3% test; dmg bundle path untested] Key licence: shipped cyanbox templates are fit on GiantSteps MTG (labels unlicensed). A clean model fit only on CC0/CC BY Freesound loops (FSL10K) is built behind a one-line switch: Beatport test exact 56.3% vs 60.3% shipped (below the 60% bar); needs attribution (Ramires et al. 2020 + CC BY uploaders). docs/analysis/key-licence-2026-10-31.md - rec: ship clean-fsl for the public beta (legal risk over 4 points), keep improving it - branch: ov-1028 (switch SHIPPED in cyankey.rs)
5. Old stash@{0} (scrub-pause waveform tweaks; prototyping/viewport only, not the app, so not built in the run) — rec: build on a branch for review — branch: none
6. `.claude/settings.local.json` allows `git push *`, which bypasses the push ask rule — rec: remove that allow line (Merlin edits it) — branch: none
8. [accepted 2026-10-23 per rec] Add vitest as an app dev dependency for unit tests (Q18, branch only) — rec: keep — branch: q18-app-tests
7. Push master (30 commits ahead, docs + analysis prototype + tests) — rec: push — branch: master

13. [accepted 2026-10-28 per rec] O1b: ×2 then ÷2 drifts the grid half a bar after a save (rescale anchors on the first downbeat); fix = additive optional `grid_anchor_ms` in edits.json — rec: keep — branch: ov-1028 (O1b)

14. [accepted 2026-10-28 per rec] O2 adds `@playwright/test` as an app devDependency for `npm run smoke` — rec: keep — branch: ov-1028

15. CI workflow refreshed for ov-1028 (macOS dmg + check, Windows nsis, Linux deb + smoke; weights bundled; actionlint clean, Linux steps pass locally, never run on GitHub) - rec: OK to push branch ov-ci for a first run - branch: ov-ci (e7e6f3f)

16. Bar 1: Rekordbox bar 1 matches the model downbeats only 61.7%; "first grid beat = bar 1" matches 78.9%; hybrid 80.0% — rec: listen to ~5 disagreeing tracks on the Mac, then pick (docs/analysis/rust-status-2026-10-28.md) — branch: none

17. [accepted 2026-10-28 per rec] O9: bundle Beat This! weights (+83 MB, MIT, licence file shipped) via tauri.bundle-weights.conf.json + bundle-app.sh; no-backend now errors visibly — rec: keep, check on the next Mac .dmg build — branch: ov-1028

18. [accepted 2026-10-29 per rec] fb-dupes adds Rust deps `tar` + `filetime` (MIT/Apache) to pioneer-library for the backup .tar.gz — rec: keep — branch: ov-1028

19. Waveform detail cache cap is 512 MiB (~100 six-minute tracks at 5 MB each) - rec: raise to 2 GiB - branch: none (q50-trackload follow-up)

20. [resolved 2026-10-31: r7 stores detail via the app's own 0.5 decode, bytes identical, +2.5% CPU] Q50: analysis stores the waveform detail from its own decode, but MP3 differs from measure_detail (symphonia 0.6 vs 0.5) - rec: hold until the app moves to symphonia 0.6 - branch: q50-analysis2 (fe78598, not merged)
21. Q50: rten 0.27 bump - logits differ up to 8.6e-5, slower - rec: drop - branch: q50-rten27
22. [accepted 2026-10-30 per rec] Q50: 8 GB Macs run 3 beat-model chunks at once (~2 GB peak, about half the model throughput) - rec: keep - branch: ov-1028

23. [accepted 2026-10-30 per rec] Q50 import: additive index idx_artwork_track (created on open, no version bump; cold track list 1.3 s → 0.1 s at 10k covers) and import commit with synchronous=NORMAL + background checkpoint (power loss right after an import can roll it back; DB stays consistent) - rec: keep - branch: ov-1028

24. Beta .dmg: bundle id is com.pioneer.test-ui (Pioneer is a trademark; also the macOS data folder keys off it) - rec: pick a neutral id before any public beta, migrate app data once - branch: none

25. [accepted 2026-10-31 per rec] Boot snapshot shows the last All Tracks view (first ~40 rows, inert until the real load); reopening the last playlist on start is not built - rec: keep All Tracks - branch: ov-1028
26. [accepted 2026-10-31 per rec] Boot snapshot is skipped below ~15k tracks (at 10k it delays real rows 100-170 ms because both lists paint waveforms; the real list lands within ~100 ms there anyway) - rec: keep - branch: to build in run item 4
28. [accepted 2026-10-31 per rec] r6 import adds an additive `pending_artwork` table (no version bump) so covers stored after the rows survive a quit - rec: keep - branch: ov-1028
29. UI headroom (r5): analysis now uses cores-2 and holds new model chunks while you scroll (500 ms idle); a running chunk (~5 s) can't be stopped, so short flicks gain little. Background QoS (E-cores only) is behind FF_ANALYSIS_QOS=background - rec: keep defaults, decide on background QoS after the Mac night measures it - branch: ov-1028
27. [built 2026-10-31 on r9-ort 383ae88, unlanded: onnxruntime CPU no faster than rten on box (8-9 vs 9.6-10 trk/min), 2x memory, first result 2x later; logits within 2e-4, grid eval identical; CoreML path untested - rec: Mac-only test on a Mac night, drop if no gain] Analysis speed is capped by the beat model on CPU (91% of time; ~8 trk/min on dev, 10k = ~18 h). Only a new runtime moves it: onnxruntime + CoreML (GPU/Neural Engine) on the Mac, or fused attention. Not bit-identical, needs the grid/key evals - rec: try onnxruntime+CoreML on a branch, measure on a Mac night, keep only if evals hold - branch: none

### Needs Merlin's ears/eyes (laptop)
- Preview, master: https://dev.tail87069a.ts.net:4200 (`preview start fourfour app-review`); dies on OOM, restart by hand.
- Playback ear check: 8 steps in docs/architecture/audio-engine-2026-10.md (branch q22-audio).

### Brother's lane
- SQLCipher keys private; no PDB reader (docs/architecture/rekordbox-import.md).
- From clone research: Booth 0.0.9 CDJ-3000 fw 3.22 fixes, baken waveform/PVB2/PSSI (MIT), cyanbox MP3 timeline rule + PWAV min heights — see docs/research/clones-2026-10-23.md "Format facts".
- Sync engine changes per docs/ui/usb-sync-spec.md (playlist scope, ownership manifest, dry-run plan, progress events, capacity, removable-drive filter, wipe guard).
- Import (r6): a cold 10k import reads 2.7 GB because lofty reads ID3 picture frames even with read_cover_art(false); an ID3 reader that seeks past pictures in pioneer-usb-writer/src/scanner.rs would cut it.
- Q6 key source PDB vs OneLibrary; .2EX / PWV6-7 placement for CDJ-3000X; CDJ-3000 round-trip of a current export.

## Queue
### Q5 Unsigned macOS build [queued] [mac] — `cargo tauri build` via mac-queue; report artifact + log; no signing. 30 min
### Q6 Key source PDB vs OneLibrary [handed to brother]
### Q8 Cross-platform CI [queued] [box] — Actions on macOS/Windows/Linux (build, test, check, unsigned tauri build). Branch only; first run on the shared repo needs Merlin's OK. 45 min
### Q14 USB sync UI [waiting on Merlin] — first attempt on ui-sync-flow (b510e70, preview :4240 when started); round 3 mockups decide the rebuild.
### Q33 Rust analyzer in the app [done, branch q33-rust-analysis 23332d0, master merged] — fourfour-analysis-rs crate; backend setting rust|python (rust when weights present); 2.36 vs 1.78 tracks/min on a loaded box; Rust emits no cues/energy (licence call). Weights: fourfour-analysis-rs/scripts/fetch-models.sh. Not merged. — branch q33-rust-analysis: lib crate fourfour-analysis-rs extracted (c3cb7a2); backend/coordinator files WIP-committed unbuilt; worktree .claude/worktrees/agent-a45ef63857b3fe37b

### Q50 Speed: lightning-fast startup, track load, import, analysis [running] [box] — Merlin 2026-10-29: "hyper optimize for speed … this needs to be lightning fast". Goal: measured budgets per path, then fix the worst first. Paths: cold/warm startup to interactive (10k-track library), track load to first audible + full waveform, import of a 1k-file folder (scan → plan → commit → UI), analysis throughput (tracks/min, per-stage profile: decode, resample, mel, ONNX, fit, key, bands, writes), UI responsiveness during all of them. Acceptance: a perf report with before/after per path, a repeatable perf suite (extend app/perf + smoke perf10k + a Rust bench), every win landed on a branch with green checks; BPM/grid/key results unchanged. Fleet split: one worker per path + one for the shared perf harness first; one writer per path set. Builds on fb-wave/fb-bulk findings (docs/analysis/grid-offset-2026-10-29.md, perf notes in journal 2026-10-29). Timebox: one overnight run. Run conditions (Merlin 2026-10-29): seat only, no API/credit runs; visible app runs on the Mac (mac-queue --visible) allowed only after 00:00 Mac local time (WITA, UTC+8); Mac on AC from bedtime. Merlin runs a Grokbot watcher that pings this thread when the seat limit resets → on that ping, resume every stopped worker (SendMessage), check limits first.

## Run
status: running (2026-10-31: PM relay 2 "keep going on own priorities"; landing b13-cleankey, b14-cleankey60 refit running) · started 2026-10-30 18:06 WITA (PM queue start, Merlin: "start all projects on the holiday queue now") · two weeks to 2026-11-16 · cap 2 · seat only (5 h windows, resume after resets) · autonomy: green work lands on ov-1028 + Mac install; never master, never push
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
mac: unreachable over ssh since 2026-10-31 (travelling); merges stay on ov-1028 with box checks, install + dmg rebuild when it is back online
Mac nights: one visible batch per week (### Visible Mac batch): 10k startup/scroll/analysis timings, .dmg install; Grok told once per batch
progress 2026-10-31: items 0-8, 10-12 merged on ov-1028 (checks green). Idle: 9 (dmg install) and the Mac batch wait for the Mac; the rest waits on Merlin (Waiting 1-3 review card, 4 key licence, 15 CI push, 27 runtime, 29 after Mac night)
### Visible Mac batch
- perf night: scripts/mac/perf-night.sh --size 10k --build (r1; startup, scroll, analysis timings) - needs the Mac online + Merlin's go
- install ov-1028 + dmg check (scripts/mac/check-dmg.sh)
Hard gates: no push, no master merge, no deploy, no CDJ/USB writes, brother's format/engine lane read-only

## Rundown
Run 2026-10-30/31 on ov-1028 (branch only, checks green): startup snapshot from 20k tracks; zoomed waveform 605 -> 3 ms after analysis; import 10k 14 -> 2.1 s cold, re-import 0.14 s; first BPM in ~3 s; analysis lock 1 s -> 1.4 ms; UI hold while scrolling; key 60.3% (bar met); USB sync round 3; CI ready on ov-ci.
Still slow: analysis 8 trk/min (beat model 91% CPU; Waiting 27).
Waiting on Merlin: sync review card, key licence (4), CI push (15), runtime (27). Mac offline: installs, dmg check, perf night.
