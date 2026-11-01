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
1. Sync round 3 (https://claude.ai/artifact/1cCJTk3r92jC6kmJmz1nnJ): panel closed → whole stick view as drop target, or a "Put on stick" strip? — rec: strip — branch: none
2. Copy a playlist to another stick: Keep updated on by default? — rec: yes — branch: none
3. Build round 3 into the app (replaces ui-sync-flow's first attempt, keeps the stick header)? — rec: yes after 1–2 — branch: ui-sync-flow
4. [accepted 2026-10-31 per rec, landed on ov-1028 3e5ec61; clean 56.3% test; dmg bundle path untested] Key licence: shipped cyanbox templates are fit on GiantSteps MTG (labels unlicensed). A clean model fit only on CC0/CC BY Freesound loops (FSL10K) is built behind a one-line switch: Beatport test exact 56.3% vs 60.3% shipped (below the 60% bar); needs attribution (Ramires et al. 2020 + CC BY uploaders). docs/analysis/key-licence-2026-10-31.md - rec: ship clean-fsl for the public beta (legal risk over 4 points), keep improving it - branch: ov-1028 (switch SHIPPED in cyankey.rs)
5. Old stash@{0} (scrub-pause waveform tweaks; prototyping/viewport only, not the app, so not built in the run) — rec: build on a branch for review — branch: none
6. `.claude/settings.local.json` allows `git push *`, which bypasses the push ask rule — rec: remove that allow line (Merlin edits it) — branch: none
7. Push master (30 commits ahead, docs + analysis prototype + tests) — rec: push — branch: master
15. CI workflow refreshed for ov-1028 (macOS dmg + check, Windows nsis, Linux deb + smoke; weights bundled; actionlint clean, Linux steps pass locally, never run on GitHub) - rec: OK to push branch r35-ci for a first run (supersedes ov-ci: refreshed for coreml, dev-bins, deb checks, webkit smoke, Windows OpenSSL; trigger now master + PRs only). Windows ships without libcrypto-3 DLL until bundled or vendored - decide after the first run - branch: r35-ci (24add8b)
16. [accepted 2026-11-01 per PM relay "don't wait for reviews", landed on ov-1028 29add96; listen check still open] Bar 1: hybrid rule (grid beat 0 = bar 1 unless model downbeat vote >=80%) built on b15-bar1 e717de0 behind BAR_ONE_RULE: Rekordbox agreement test 61.5% -> 79.3%, dev 57.8% -> 88.9%; grid/BPM unchanged; 72 tracks differ (new right 59, old right 13), listen list in docs/analysis/rust-status-2026-10-28.md B15 - rec: listen to the 5 regressions on the Mac, then land - branch: b15-bar1
19. Waveform detail cache cap is 512 MiB (~100 six-minute tracks at 5 MB each) - rec: raise to 2 GiB - branch: none (q50-trackload follow-up)
21. Q50: rten 0.27 bump - logits differ up to 8.6e-5, slower - rec: drop - branch: q50-rten27
24. Beta .dmg: bundle id is com.pioneer.test-ui (Pioneer is a trademark; also the macOS data folder keys off it) - rec: pick a neutral id before any public beta, migrate app data once - branch: none
29. UI headroom (r5): analysis now uses cores-2 and holds new model chunks while you scroll (500 ms idle); a running chunk (~5 s) can't be stopped, so short flicks gain little. Background QoS (E-cores only) is behind FF_ANALYSIS_QOS=background - rec: keep defaults, decide on background QoS after the Mac night measures it - branch: ov-1028
27. [built 2026-10-31 on r9-ort 383ae88, unlanded: onnxruntime CPU no faster than rten on box (8-9 vs 9.6-10 trk/min), 2x memory, first result 2x later; logits within 2e-4, grid eval identical; CoreML path untested - rec: Mac-only test on a Mac night, drop if no gain] Analysis speed is capped by the beat model on CPU (91% of time; ~8 trk/min on dev, 10k = ~18 h). Only a new runtime moves it: onnxruntime + CoreML (GPU/Neural Engine) on the Mac, or fused attention. Not bit-identical, needs the grid/key evals - rec: try onnxruntime+CoreML on a branch, measure on a Mac night, keep only if evals hold - branch: none
31. Schema changes for 50k speed + robustness (r12/r13): store folded artist/title keys (import duplicate plan 1-1.5 s at 50k), move covers out of library.db (7.5 GB at 50k, slow commits), store analysis failures (no retry every launch) - rec: one schema bump with all three after the holiday - branch: none
32. [accepted 2026-11-01 per rec, landed on ov-1028 3248199] Analysis order after restart is not persisted (resume goes by id) - rec: side file in app data with the last run's order, no schema - branch: r21-order (not built yet)
33. Error messages (r18): 42 messages rewritten via app/src/lib/userErrors.ts, 13 silent failures surfaced. Real bugs inside: analyzeTrackIds/analyzeTracks and playlist saves fall back to the browser mock in the real app (fake "Analyzed" events, unsaved playlists look saved); player errors never rendered. Doc docs/ui/error-messages-2026-11.md - rec: land first thing after the holiday (bugs are real); open calls: library.db open panic needs a recovery screen, status bar gets clobbered by analysis progress (toast?) - branch: r18-errors
34. Keyboard + screen reader (r17): list is a real listbox (arrows, Shift range, Enter plays, Shift+F10 menu), sidebar/menus/dialogs keyboard-reachable with focus trap + return, CUE by keyboard, 26 keyboard specs. Behaviour change: Tab/Space yield to keyboard-focused controls (mouse flows unchanged). Touches prototyping design-system (commit af739a7, AGENTS fence) - rec: land after a Mac WebKit check; open: keyboard reorder, waveform seek, rating by keys - branch: r17-a11y
35. Cover cache cap (r31): 512 px covers LRU 96 with pins (player, inspector, organize rail/picker, drop flash); organize views now load only the covers they draw. Visible: the rail's "+N" counts tracks with artwork, not covers loaded so far - rec: land after a quick look (rapid selection: inspector, player, organize rail never blank) - branch: r31-cover-cap
36. WebKit startup (r29): the design-system reset `*, *::before, *::after { box-sizing }` makes WebKit resolve two pseudo styles per element; `* { box-sizing }` cuts row style time ~40%, WebKit 10k first row 1.84 -> 1.37 s (loaded box), screenshots byte-identical. Edits prototyping/viewport/design-system/base.css (AGENTS fence) - rec: allow and land - branch: r29-mac-startup (commit c89aeff)
### Needs Merlin's ears/eyes (laptop)
- Preview, master: https://dev.tail87069a.ts.net:4200 (`preview start fourfour app-review`); dies on OOM, restart by hand.
- Playback ear check: 8 steps in docs/architecture/audio-engine-2026-10.md (branch q22-audio).
### Brother's lane
- r20 Windows risks: is_removable true for every drive, eject via umount, statvfs stub unlimited, pdb.rs extension match case-sensitive (TRACK.MP3 -> type 0); see Platforms 2026-11 in robustness-2026-11.md.
- r12: USB export puts two tracks with the same artist + file name (untagged "01 Intro.mp3") on one path; repro ignored test in pioneer-library/tests/robust_store.rs. Also lofty rejects some bad tags so playable files are skipped (relaxed parse in scanner.rs).
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

30. Clean key model v2 (b14): self-trained on CC BY FMA clips; Beatport test 57.3% vs 56.3% (2 tracks, within noise; dev +9), adds 264 FMA artist credits. The 36-weight template model is at its ceiling without ~1,500 permissively labelled tracks, so 60% clean is out of reach this way - rec: leave unlanded, ship CLEAN_FSL, accept ~56% for the beta - branch: b14-cleankey60 (11784f5)

## Run review
Always on (user systemd ff-review.service + ff-app-review.service, Restart=always, survive reboots; 4210 serves the last build in ~/.cache/preview/fourfour-app, rebuild it after merges). Checked 2026-11-01 over Tailscale (HTTP 200, real content):
- https://dev.tail87069a.ts.net:4220/lab.html - review cards (5 open: status chip, sidebar, skeleton, scroll while analysing, USB sync round 3 with 9 screenshots); Keep / Change (note) per card
- https://dev.tail87069a.ts.net:4210/ - the app UI on ov-1028 in the browser (no Rust backend, mock data)
- Mac: ov-1028 installed in /Applications/fourfour.app (12:21 WITA); dmg at ~/dev/build/fourfour-mac/build/mac/dmg-20261101-1221/

## Run
status: running until Merlin says stop (2026-11-01: no reviews during holiday, take recs; own-priority backlog 13-22, refill before dry; Mac visible batch held for Merlin's go tonight); perf night HELD until Merlin's go tonight (Merlin 12:21: no UI takeover now)) · started 2026-10-30 18:06 WITA (PM queue start, Merlin: "start all projects on the holiday queue now") · two weeks to 2026-11-16 · cap 2 · seat only (5 h windows, resume after resets) · autonomy: green work lands on ov-1028 + Mac install; never master, never push
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
Own-priority backlog (Merlin 2026-11-01: "fill the queue with your own prios so you never run out of work"; refill before it runs dry):
13. r11-coreml: build r9-ort with CoreML on the Mac headless + 3-track sanity now; full analysis bench tonight in the Mac batch [built, branch r11-coreml 6542279: Mac ort link fix (build.rs clang_rt), coreml-bench.sh; sanity unrun, Mac slept; first check ~/.cache/r11-work/sanity/ on mbp] [-> r24-coreml 2eaed24: KEEP. Mac dev bench: CoreML w2t2/w4t2 26-32 trk/min vs rten 12-14 (2.2x), grid/BPM identical 49/49, RSS 3.7-4.3 GB vs rten 3.5-5.2 GB; ship via r26]
14. r12-robust: import/analysis/sync with bad input (zero-byte, corrupt, unsupported, missing, read-only, huge files, odd tags/unicode paths); tests + fixes [landed ea0ddaa; open items in docs/architecture/robustness-2026-11.md]
15. r13-50k: startup, scroll, search, import and memory at 50k tracks on the box; fix the worst regressions [landed f609c89; startup 50k still 2.5-3.2 s, rest in perf-run R13]
16. r14-resume: analysis queue across restarts and crashes at 10k (no lost/duplicated work, steady memory over hours) [landed a7131e7; docs/architecture/analysis-resume-2026-11.md]
17. r15-reveal: staged waveform reveal on a branch (unlanded; only if the Mac night shows first expand >50 ms)
18. r16-quant: beat model fp16/int8 weights experiment (prototype only, grid/key evals gate it) [dropped; branch r16-quant b7a274f: int8 -7% CPU, grid 43 vs 44/50; no VNNI, attention stays fp32; CoreML (r11) is the lever]
19. r17-a11y: keyboard-only pass over library, player, sync (branch, unlanded) [built, branch r17-a11y e83fdd3; Waiting 34]
20. r18-errors: user-facing error messages audit (branch, unlanded; copy for Merlin) [built, branch r18-errors abae25a; Waiting 33]
21. r19-tests: Rust + vitest coverage gaps in sync/import/analysis store; flaky test hunt [landed d4b4ef3; 52 tests, vitest timeout fix, no bugs]
22. r20-winlinux: Linux .deb + Windows cross-check builds and smoke on the box [landed 19fc7bd; deb 116 MB + xvfb smoke ok, Windows cargo check clean]
23. r21-order: persist the last run's analysis order in a side file (no schema) so resume keeps selection/visible-first; sweep stale waveform-cache .part files at startup (Waiting 32 rec) [landed 3248199]
24. r22-startup: 50k startup to interactive under 1.5 s (lazy row proxies, smaller load_state payload; R13 profile, no schema) [landed 240d8cb; 50k warm 1.35 s, cold 1.57 s, first 1.85 s; payload 21.6 -> 8.4 MB]
25. r23-memory: long-session soak on the box (hours of scroll/search/analysis, RSS + JS heap drift, leaks) [landed 30275a0; 1 leak fixed (artwork row thumbnails), rest flat]
26. r25-package: release bundles without dev bins, deb Depends libasound2, Windows path names in the frontend, app-code platform guards [landed f82e56e; deb deps unverified until r28]
27. r26-coreml-ship: CoreML as the Mac analysis default (w2 t2), rten fallback if CoreML fails to load, ort in the Mac bundle (min macOS 13.4), dmg check; memory per session [landed fae2186; app 1.4x on 12 tracks, peak RSS 7.0-7.7 GB vs rten 6.2 -> cap bridge threads follow-up]
28. r27-dbsafe: library.db open never inits over real data (unreadable schema_version), no setup panic on a damaged DB [landed 5d7df87; recovery screen left for Merlin]
29. r28-deb-verify: real Linux bundle + dpkg-deb -I + ldd to confirm deps and no dev bins (after r25) [landed 8511026; deb 90 MB, Depends + contents verified]
30. r29-mac-startup: Mac WKWebView 10k startup first row 1.8-2.8 s / interactive 2.2-3.8 s (box 0.8-1.1 s): profile on the Mac, fix the gap [harness + doc landed on ov-1028 a53c297; CSS fix parked on r29-mac-startup c89aeff, Waiting 36]
31. r15 trigger hit: Mac cold first expand longest frame 472-520 ms in 3/5 launches -> build staged reveal on a branch (r15-reveal) [parked, branch r15-reveal 00acf20: longest frame -20-40% but click-to-drawn worse (WebKit +0.7 s), not worth landing; its probe metrics (longest_busy, pending_frames) tell next Mac night if the stall is outside the page]
32. r30-coreml-mem: cap the rten bridge so CoreML peak RSS <= rten (7.0-7.7 -> <=6.2 GB) [landed 7dba30c; 6.1-6.2 GB, latency kept; accepted per rec, last 0.3 GB vs same-session rten left]
33. r31-cover-cap: cap the 512 px artwork.cache (UI-aware eviction, never blank a visible image) [built, branch r31-cover-cap 857496a; Waiting 35]
34. r32-probe: port r15's first-expand probe metrics + webkit smoke option to ov-1028 (no behaviour change) [landed]
35. r33-dupplan: import duplicate plan at 50k (1-1.5 s) without schema: in-memory folded-key index kept across imports [landed; later imports 50k 990 -> 74 ms]
36. r34-webkit-idle: WebKit idle scheduler (no rIC), Icon parse once, lighter skeleton rows; app code only, no prototyping edits [running]
37. r35-ci: refresh ov-ci workflow for coreml/dev-bins/deb deps/webkit smoke (branch, push needs Waiting 15) [built, branch r35-ci 24add8b]
review: https://dev.tail87069a.ts.net:4220 (phone review cards: lab/, add with lab/scripts/review-shot.ts + hub:card, read answers with hub:notes) · app preview https://dev.tail87069a.ts.net:4210 (ov-1028, no Rust backend; always on via user systemd ff-review / ff-app-review)
each finished user-visible item gets a review card; when emberland says @lab/kit (lab-infra packages/lab-kit) landed, swap lab/ inbox UI to mountInbox (keep cards/media/capture script; no name-based logo, working title); Grok gets the 4220 link
mac: unreachable over ssh since 2026-10-31 (travelling); merges stay on ov-1028 with box checks, install + dmg rebuild when it is back online
Mac nights: one visible batch per week (### Visible Mac batch): 10k startup/scroll/analysis timings, .dmg install; Grok told once per batch
progress 2026-10-31: items 0-8, 10-12 merged on ov-1028 (checks green). Idle: 9 (dmg install) and the Mac batch wait for the Mac; the rest waits on Merlin (Waiting 1-3 review card, 4 key licence, 15 CI push, 27 runtime, 29 after Mac night)
### Visible Mac batch
- perf night: scripts/mac/perf-night.sh --size 10k --build (r1; startup, scroll, analysis timings) - needs the Mac online + Merlin's go
- coreml bench (headless, but long and heavy): on mbp in ~/dev/build/fourfour-r11, `scripts/mac/coreml-bench.sh --split dev --reps 2 --rten "2 6;1 12" --coreml "1 1;2 1;2 2;4 2" --rounds 7` (1-2 h); keep/drop rule in perf-run-2026-11.md R11
- [done 2026-11-01] install ov-1028 + dmg check (headless, all PASS: build/mac/dmg-20261101-1221 on the Mac)
Hard gates: no push, no master merge, no deploy, no CDJ/USB writes, brother's format/engine lane read-only

## Rundown
Run 2026-10-30/31 on ov-1028 (branch only, checks green): startup snapshot from 20k tracks; zoomed waveform 605 -> 3 ms after analysis; import 10k 14 -> 2.1 s cold, re-import 0.14 s; first BPM in ~3 s; analysis lock 1 s -> 1.4 ms; UI hold while scrolling; key 60.3% (bar met); USB sync round 3; CI ready on ov-ci.
Still slow: analysis 8 trk/min (beat model 91% CPU; Waiting 27).
Waiting on Merlin: sync review card, key licence (4), CI push (15), runtime (27). Mac offline: installs, dmg check, perf night.
