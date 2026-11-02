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
36. WebKit startup (r29): the design-system reset `*, *::before, *::after { box-sizing }` makes WebKit resolve two pseudo styles per element; `* { box-sizing }` cuts row style time ~40%, WebKit 10k first row 1.84 -> 1.37 s (loaded box), screenshots byte-identical. Edits prototyping/viewport/design-system/base.css (AGENTS fence) - rec: allow and land (r45 confirmed in a full build: WebKit row style -32%, pixel-identical; a second fenced step - cell geometry as per-column custom properties - gives ~10% more) - branch: r29-css-v2 (or r46-dsrows = r29 fix + cell geometry patch; rec: land both together)
37. Library recovery panel (r38): when library.db fails to open, the list shows why (newer version / damaged / in use) plus Open another library, Restore a backup, Start a new library, Try again (locked); failed file never touched; screenshots ~/.cache/r38/*.png on dev. Copy uses the working title ("newer version of fourfour") - rec: land after review, swap the name for "the app" - branch: r38-recovery
Note on 33/34/35/36/37: land the -v2 branches (current with ov-1028), or parked-all-v2 for all of them at once (r18+r38 initLibrary conflict resolved there).
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
status: running until Merlin says stop · holiday 2026-10-30 to 2026-11-16 · cap 2 · seat only · no reviews during holiday, take recs, log decisions here · green work lands on ov-1028 (+ Mac install); never master, never push · Mac: only on Merlin's go (last go 2026-11-01 21:42 for that night)
targets: beta-ready on ov-1028, as fast as possible at 10k+ (startup, analysis, track load, scroll); speed first, then beta
done so far: plan items 0-12 and own-priority r11-r41 (detail: docs/state-archive-2026-10-28.md "Run plan as of 2026-11-02", journal, docs/architecture/perf-run-2026-11.md R-sections)
open own-priority items:
- r42-realsoak: 45-60 min soak of the real Linux app under xvfb [running]
- r44-version: [landed] app version now from the app crate (shown version 0.9.14 -> 0.1.0, matching the dmg; decided: lands, it corrects a wrong number); memo keyed on ANALYSIS_VERSION (bump by hand when analysis changes)
- r45-rowcost: [landed] no app-side lever; all per-row WebKit gains are in the fenced design system (reset fix -32% style, cell geometry as custom properties ~10% more); bench_startup build fixed
- r46-dsrows: [parked, branch r46-dsrows 220f237 on r29-css-v2] cell widths as table-level custom properties; pixel-identical incl. resize/reorder; resize touches 0 body attributes (was 560); per-row gain within noise
parked for Merlin (land the -v2 branches, current with ov-1028; parked-all-v2 a87f361 = all at once): r18-errors-v2 (Waiting 33), r17-a11y-v2 (34), r31-cover-cap-v2 (35), r29-css-v2 (36), r38-recovery-v2 (37); r15-reveal (not worth landing), r16-quant (dropped), r35-ci (Waiting 15)
next Mac night (needs go): perf-night.sh --size 10k --expand-runs 5 on ov-1028 (now with CoreML, font preload, error kinds; probe has longest_busy/pending_frames/page origin); compare r15-reveal and r29-css-v2; 50k only with >= 40 GB free (script now cleans up and stops when low)
review: https://dev.tail87069a.ts.net:4220 (cards) · https://dev.tail87069a.ts.net:4210 (app on ov-1028, mock data; rebuilt after merges)
Hard gates: no push, no master merge, no deploy, no CDJ/USB writes, brother's format/engine lane read-only

## Rundown
Run 2026-10-30 to 11-02 on ov-1028 (checks green): Mac analysis on CoreML 2-3x faster with identical grids (33 vs 10.5 trk/min on 205 tracks), peak RSS at rten level; 50k startup 2.2 -> 1.35 s warm; WebKit post-first-row stall fixed (font preload); import duplicate plan at 50k 990 -> 74 ms; analysis survives quit/kill/restart and remembers order + failed files; damaged library.db never overwritten; Linux deb verified; Windows compiles; WebKit smoke green; error kinds on commands.
Mac numbers (2026-11-01): 10k warm first row 1.6-1.9 s (fresh HOME each run), scroll during analysis p95 18 ms, first expand stall ~500 ms in 3/5.
Waiting on Merlin: parked -v2 branches (33-37), schema bump (31), CI push (15), sync round 3 (1-3), key licence (4), bar-1 listen (16), bundle id (24).
