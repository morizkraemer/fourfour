# fourfour state archive (2026-10-28)

Detail moved verbatim out of docs/STATE.md on 2026-10-27 (housekeeping, Q47): landed queue entries Q1–Q41, fixed brother-lane items, the preview link for an old branch, and the old Rundown. Open items stay in STATE.md.

## Queue (landed / done)
### MERGED 2026-10-26 into master: q40-rb-match (whole app stack + waveforms) and q30-grid (analysis prototype). Full checks green (Rust tests, 172 app tests, check, build). CDJ checks for the brother: docs/architecture/cdj-checklist-2026-10.md.
### Q1–Q4, Q7, Q9–Q13 [landed] — playback, svelte-check 0, export tests, browser preview, benchmark, cache fix, packaging decision, key detector (57%), BPM/grid (BPM ±0.1 86%, grid ±20 ms 47%). See journal + docs/analysis/.
### Q15 Grid phase + drift [done, branch q15-grid 8b9a3d0; doc landed] — grid ±20 ms 47%→72% (148/205), BPM ±0.1 86%→88%; full-track Beat This! + tempo comb + integer snap + mp3 encoder-delay fix; ~25 s/track. Code stays on branch (tight); mp3-delay rule needs a hardware check (brother).
### Q16 Key detector round 2 [done, branch q16-key a6b4929; doc landed] — still 57.3% exact (70.9% incl. relative+fifth), n=199; augmentation/MLP/segments within noise. Next: check Beatport label noise, more permissive training data, a parallel-key feature. Weights gitignored (CC BY-SA, measurement only).
### Q17 Doc drift cleanup [landed ac933c8] — own priority: AGENTS.md test-suite + stratum/src/analyzer claims, pioneer-test-ui README, pipeline-handoff status, docs/README index for the new analysis/ui docs.
### Q18 App unit tests [done, branch q18-app-tests 4dba72d, rebased on master] — 41 vitest tests (analysis validators, mock, library helpers, menus), check + build green; npm audit advisories not reviewed; untested: mapTrackInfoToLocal, normalizeTrackInput, sortTracks (need export/extraction)
### Q19 Clone research (item 7) [done] — docs/research/clones-2026-10-23.md; "lite" = rbxport (GPL, ideas only); copyable: cyanbox (BSD-2) key/beat/export, baken (MIT) waveform/PVB2/PSSI/layout
### Q20 Startup + UI speed (items 1, 11) [done, branch q20-speed 02933a0; doc landed] — load_state 30k 24.7 s→0.5 s; first paint 30k 188 s→3.3 s; virtualised table (39 DOM rows); 50 Rust tests green. Left: waveform canvas jank (design system), sync load_state blocks ~0.5 s, 25.6 MB payload.
### Q22 Audio engine review (item 10) [done, branch q22-audio c21df16; doc landed] — 5 high fixes (seek/stop race, L/R shift on underrun, zero-rate WAV panic, truncated file stuck playing, silent after failed device switch) + 6 medium; 26 new tests incl. 0-alloc callback + thread spam; load→audio 1.6 ms, seek→audio 2.5 ms (fake sink). Needs Mac check (8 steps in doc).
### Q23 Fast track + waveform load (item 4) [done, branch q23-load 27a4372 (contains q20-speed); doc landed] — get_analysis_data 137→3.8 ms, select→detail ~300→50 ms at 30k, load→waveform 325→80 ms; async load_state/analysis/artwork; LRU + prefetch. Conflicts with q22-audio in player.svelte.ts + main.rs (merge order: q23 then q22). Not run in a real window.
### Q24 Rekordbox library import (item 6) [done, branch q24-rb-import c31a860 = q23 + q22 + import; doc landed] — master.db, OneLibrary stick, rekordbox.xml; your library: 631 tracks found + imported, 607 grids, 3 playlists; merge keeps your edits, never deletes. Import UI (Settings → General) unreviewed.
### Q25 Multi-USB sync (item 3) [done, branch q25-multi-usb 04fe899 (contains q24); doc landed] — `sync_usb_many`: prepare once, one thread per stick, failures isolated, root + free-space checks, per-stick progress; 3 sticks 2.4–3× faster than serial; playlist-scoped selection in the library layer; 8 tests. No UI.
### Q28 Waveform drawing + scroll perf [done, branch q28-waveform-perf 2a7a232 (contains q25); doc landed] — scroll CPU/frame −21%, paint 24→3 ms, getComputedStyle per scroll 512→0, playhead layout 0.8→0.03 ms/tick; pixel diff ≤7/255; 512 px cached artwork thumbnails. Wall-clock win unproven (box load 20–33).
### Q29 Track row mount + 30k paint cost [done, branch q29-rows 35dd87f = integration of q20→q23→q22→q24→q25→q28→q29; doc landed] — fling CPU/frame 166→74 ms (pooled rows), layerize while playing 5.2→3.9 ms; row pixels identical. DOM row order is now slot order (drop index adapted).
### Q32 First-search spike + row-window helper [done, branch q32-search ae9571c = top of the app stack; doc landed] — first keystroke at 30k 445→120 ms, later 128→94 ms; search index built in idle time; rowWindow.ts + 20 tests (108 app tests).
### Q31 App tests on the integration stack [done, branch q31-tests f8488e8 = ALL app/backend work tonight + vitest] — 88 app tests (47 new) + all Rust tests green. Preview of this branch: https://dev.tail87069a.ts.net:4250 (try ?mock=30000).
### Q27 Repro tests for writer bugs [done, branch q27-writer-repro 450ea04; doc landed] — PDB export fails from 192 tracks (137 with 200-char titles); ANLZ colour detail wrong from 437 s (4 overflow sites, anlz.rs:258/314/364/404). Ignored repro tests on branch. docs/architecture/writer-bugs-2026-10.md
### Q21 Analysis review pass 1 (items 8, 2, 5) [done, branch q21-analysis 1fc254c (contains q15-grid); doc landed] — unified analyzer `ffa` (decode once, all stages, memory-capped pool): ~4.8 tracks/min on 8 workers, 0.56 GB/worker; Beat This! = 93% of CPU; grid 72.2% held, BPM ±0.1 88.8%; key stage now the cyanbox port 58.3% (n=199). Licence flag: cyanbox key templates fit on GiantSteps MTG (annotations unlicensed).
### Q26 Analysis review pass 2 (item 8) [done, branch q26-analysis cdc4f30 (contains q21); doc landed] — CPU 74→70 s/track, 5.3 tracks/min at 8 workers, tempo scan 1.1 s→0.19 s; native-rate decode, stratum removed; grid 72.2% / BPM 87.8% / key 58.3%; ONNX Runtime only 1.3–1.5× → not adopted. Lame3.100 mp3s are one frame late (fix needs dev check).
### Q30 Grid round 3 (item 5) [done, branch q30-grid 260a51e = top of the analysis stack; doc landed] — grid ±20 ms 72.2%→78.5% (161/205), BPM ±0.1 87.8% held; Lame3.100 files are a frame EARLY-fix (−26 ms), wav/flac −10 ms shift; half-beat phase: no usable signal.
### Q34 3-band export [done, branch q34-export-waveforms 7a60879] — overflow fixed (u64, long-track tests pass to 60 min); real bands (baken port, MIT, NOTICE) → PWV7/PWV6/PWVC in new .2EX, PWV5 real u16 colour, PWV2 added; vs Rekordbox held-out: PWV7 corr 0.96/0.98/0.89, PWV3/5 heights 99.7% exact when computed from audio. PNGs: scratchpad/waveforms/export/. PWV6 kept ≤127 (Rekordbox does), not 45.
### Q35 3-band app rendering [done, branch q35-app-waveforms df5ba3d] — one renderer for rows, detail, compact, overview, zoom; low in front; tokens --ff-wave-*; 143 app tests, 60/60 pixel checks; fling +10% CPU; rows still approximated from PWAV. Screenshots: scratchpad/waveforms/app/{before,after,compare}.
### Q36 Waveform integration [done, branch q36-waveforms fdbb651 = q34 + q35] — app draws the stick bytes (PWV7/PWV6), 128-col 3-band row previews; PWV3 heights 18.6→99.7% exact; fling regression gone, expanded views −8..−30% CPU; side-by-side vs Rekordbox: scratchpad/waveforms/final/. Over budget: backend load_state 30k 480→910 ms, row select 19→42 ms; our high band weaker than Rekordbox.
### Q37 Waveform polish [done, branch q37-waveforms ae7bb44 = final waveform branch] — high band vs Rekordbox exact 13.8→41.7%, corr 0.89→0.93; load_state 30k 850→350 ms; row select 42→20.7 ms; schema v4 (row_preview BLOB, additive) needs your OK.
### Q38 Row preview = simple loudness envelope [done, branch row-wave-variants e2fb03b (on q37-waveforms)] — Merlin picked variant e; 3-band stays in players; 159 app tests. Gain/curve tuned on mock only.
### Q39 Waveform fix from Merlin's Mac test [done, branch q39-wave-fix 7401e1d (on row-wave-variants)] — flat block = beat-long peak pooling; layer order low back/mid/high core; whole-pixel scroll fixes the flashing; detail strip 3-band; legacy payload path fixed too. Installed on the Mac as fourfour.app.
### Q40 Waveforms matched to Rekordbox 1:1 [done, branch q40-rb-match eb52c59 (round 3: overview from a PWV6-style pass, corr .99 per band; per-track level ~16% off) (on q39-wave-fix)] — on Rekordbox's own bytes our renderer matches its screenshots (zoom edge error 17→2 px; bottom-anchored overview); our data: per-track gain on Gate ~13% low, Dubby mp3 27 ms offset. Being installed on the Mac.

## Brother's lane (fixed)
- **CRITICAL (Q25): `pdb.rs::split_into_pages` panics at ~200+ tracks (u16 heap offsets wrap past 64 KiB) → write_usb/sync_usb fail on any real library. Fix: usize offsets, rebase per chunk.**
- Bug from Q24: writer/anlz.rs colour-detail index `i * src_len / entry_count` overflows u32 on tracks > ~7 min (panics in debug). Also: parse_pco2 drops cue names/colours; SQLCipher keys private; no PDB reader (docs/architecture/rekordbox-import.md).
- `get_mounted_volumes` lists the system disk ("Macintosh HD") and offers Wipe: must fix.

## Needs Merlin's ears/eyes (old preview)
- Preview of tonight's combined branch (q31-tests; q32-search adds the search fix): https://dev.tail87069a.ts.net:4250 (?mock=30000 for a big library)

## Old Rundown
Overnight: startup 30k 24.7 s→0.5 s backend / 188 s→3.3 s first paint; track load ~300→50 ms; fling CPU −55%; search first key 445→120 ms; audio engine 11 fixes + 26 tests; Rekordbox import (master.db/USB/XML); multi-USB sync 2.4–3×; grid 47→78.5%; analysis CPU 74→70 s/track; 108 app tests. All on branches (tight); docs on master.
Critical for the brother: USB export fails from 192 tracks; colour waveform wrong > 7:17 (docs/architecture/writer-bugs-2026-10.md).

## Queue entries archived 2026-10-30
### Q42 Stick sync for real [done, branch q42-sync d56f471, merged into q42-overnight 51a9ef0] [box] — sync button → sync_usb_many: stick All tracks + chosen playlists (SyncSelection::Playlists track_ids), stable stick id (not path), capacity check, dry-run plan shown before write. Fake stick folder only; no real USB writes. Validation + headless. 180 min
### Q43 Review rounds 3–6 [done, q43-review 33b2906 → q42-overnight 8d84da9; 8 fixes; follow-ups in Q43b] [box] — bug hunt over the q41-feedback merges; extra care: wipe guard, volumes.rs, writer pdb/cue changes, tempo engine RT safety. Fix clear bugs on branch, list the rest. 150 min
### Q44 Leftovers [done, q44-leftovers a26ab3c → q42-overnight 6f654ab] [box] — row waveforms blue-block (row_bands detail labelled overview), player overview fallback → stripBandsFromAnalysis, flaky vitest (find + fix). 90 min
### Q45 Unsigned .dmg [done] — Mac ~/dev/build/fourfour-dmg/target/release/bundle/dmg/fourfour_0.1.0_aarch64.dmg (32 MB, checksum valid, ad-hoc signed, arm64 only, app 79 MB incl. dev bench binaries) [mac] — `cargo tauri build` .dmg on the Mac build clone (separate dir), launch smoke check headless; report artifact + size + log. No signing/notarising. 60 min
### Q43b Review follow-ups [done, q43b-fixes dab9629 → q42-overnight 0c713b9] [box] — PDB oversized row, fake data on error paths, Linux removable filter + wipe path check, Esc/editable/listener/loadStick/claim_selection. 120 min
### Q46 Key refit on permissive labels [done, branch q46-key c6deeff, report only] — best licence-clean 55.8% exact (cyanbox 58.3%), bar 60% not met; doc docs/analysis/key-licence-2026-10-27.md [box] — find permissively licensed key labels, refit templates, measure on Beatport set; report only, never shipped. 150 min
### Q48 Tempo engine robustness [done, q48-tempo-robust 8c2dd93 → q42-overnight dc818de] [own prio: bugs] [box] — stretcher built at device sample rate, crossfade on stretch→varispeed dropout, ring size for 8ch/192k, NaN sanitising. 120 min
### Q49 [done, branch q49-key-midi d0a176b, never landed: 55.8% test, ties Q46] Key: render permissive MIDI (Lakh CC BY / POP909 MIT) via FluidR3 (MIT) for template fit [own prio: experiment, never landed] [box] 150 min
### Q47 Housekeeping [done, a72200e: 27 worktrees removed, STATE 96→64 lines, disk 47%] [box] — remove merged worktrees + caches (check uncommitted/ignored first), disk < 70%; trim STATE.md to ≤80 lines (history → journal/docs). 45 min

## Waiting on Merlin, archived 2026-10-31 (accepted/resolved)
9. [accepted 2026-10-27 per rec] Q42: library schema v5→v6 additive (sticks, stick_tracks, stick_playlists) — rec: keep — branch: q42-overnight
10. [accepted 2026-10-27 per rec] Q42: writer `sync_usb_keeping(keep_files)` so removals never delete files fourfour didn't write (brother's lane, no format bytes) — rec: keep, tell brother — branch: q42-overnight
11. [resolved 2026-10-28: Merlin tested, merged to master 9fc5e48] Review on the Mac: stick board, sync dialog, ribbon slot, ⌥ pads, deep zoom, new tempo bar, search palette, playlist folders + right-click, import dialog, rating/colour columns — rec: test, then merge q42-overnight — branch: q42-overnight
12. [accepted 2026-10-28 per rec] schema v6→v7 additive (playlists parent_id, is_folder); folders exported as PDB/OneLibrary tree, untested on a CDJ — rec: keep, brother checks on hardware — branch: q42-overnight
0. [resolved 2026-10-27] Writer bugs (192-track PDB split, >7:17 colour overflow, cue names/colours) fixed on wu-writer, merged to master; CDJ check still open: docs/architecture/cdj-checklist-2026-10.md — rec: brother runs the checklist — branch: master
8. [accepted 2026-10-23 per rec] Add vitest as an app dev dependency for unit tests (Q18, branch only) — rec: keep — branch: q18-app-tests
13. [accepted 2026-10-28 per rec] O1b: ×2 then ÷2 drifts the grid half a bar after a save (rescale anchors on the first downbeat); fix = additive optional `grid_anchor_ms` in edits.json — rec: keep — branch: ov-1028 (O1b)
14. [accepted 2026-10-28 per rec] O2 adds `@playwright/test` as an app devDependency for `npm run smoke` — rec: keep — branch: ov-1028
17. [accepted 2026-10-28 per rec] O9: bundle Beat This! weights (+83 MB, MIT, licence file shipped) via tauri.bundle-weights.conf.json + bundle-app.sh; no-backend now errors visibly — rec: keep, check on the next Mac .dmg build — branch: ov-1028
18. [accepted 2026-10-29 per rec] fb-dupes adds Rust deps `tar` + `filetime` (MIT/Apache) to pioneer-library for the backup .tar.gz — rec: keep — branch: ov-1028
20. [resolved 2026-10-31: r7 stores detail via the app's own 0.5 decode, bytes identical, +2.5% CPU] Q50: analysis stores the waveform detail from its own decode, but MP3 differs from measure_detail (symphonia 0.6 vs 0.5) - rec: hold until the app moves to symphonia 0.6 - branch: q50-analysis2 (fe78598, not merged)
22. [accepted 2026-10-30 per rec] Q50: 8 GB Macs run 3 beat-model chunks at once (~2 GB peak, about half the model throughput) - rec: keep - branch: ov-1028
23. [accepted 2026-10-30 per rec] Q50 import: additive index idx_artwork_track (created on open, no version bump; cold track list 1.3 s → 0.1 s at 10k covers) and import commit with synchronous=NORMAL + background checkpoint (power loss right after an import can roll it back; DB stays consistent) - rec: keep - branch: ov-1028
25. [accepted 2026-10-31 per rec] Boot snapshot shows the last All Tracks view (first ~40 rows, inert until the real load); reopening the last playlist on start is not built - rec: keep All Tracks - branch: ov-1028
26. [accepted 2026-10-31 per rec] Boot snapshot is skipped below ~15k tracks (at 10k it delays real rows 100-170 ms because both lists paint waveforms; the real list lands within ~100 ms there anyway) - rec: keep - branch: to build in run item 4
28. [accepted 2026-10-31 per rec] r6 import adds an additive `pending_artwork` table (no version bump) so covers stored after the rows survive a quit - rec: keep - branch: ov-1028


## Run plan as of 2026-11-02 (archived from STATE)

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
36. r34-webkit-idle: WebKit idle scheduler (no rIC), Icon parse once, lighter skeleton rows; app code only, no prototyping edits [landed e2f36d9: late font-subset swap (U+0301 in Cyrillic subset) fixed by preloading faces; WebKit longest frame after first row 190-644 -> 24-122 ms]
37. r35-ci: refresh ov-ci workflow for coreml/dev-bins/deb deps/webkit smoke (branch, push needs Waiting 15) [built, branch r35-ci 24add8b]
38. r36-covers: import covers step 1.2-1.4 s per 1k files, faster with identical stored bytes, no schema [done: at SQLite floor (commit of ~190 MB WAL per 1k), no code change; byte-identity test + doc landed 817d347; real lever = covers out of library.db (Waiting 31)]
39. r37-webkit-smoke: 17 smoke specs fail under FF_SMOKE_BROWSER=webkit on ov-1028 (topbar 14, leaks, popups, sticks): fix test or app, since WKWebView is the Mac target [landed 2faba7f: no app bugs; 16 = benign ResizeObserver pageerror only WebKit reports, 1 = CDP-only API; plus a hot-cue drag race fixed; WebKit smoke 113 green]
40. r38-recovery: recovery panel when library.db fails to open (open another, restore backup, start new, try again if locked); branch only, visible [built, branch r38-recovery 2e8b92a; Waiting 37]
41. r39-errkinds: machine-readable error kinds on command errors (strings unchanged) so r18/r38 stop matching error text [landed; 15 kinds, ~30 commands; sync engine sentences classified at our boundary (brother: add a kind to StickReport)]
42. r40-refresh: -v2 copies of parked branches (r18, r17, r29-css, r31, r38, r15) merged with current ov-1028 + an all-together branch, so they land cleanly after the holiday [done: r18-errors-v2 1dad3c4 (uses error kinds), r17-a11y-v2 93f07e0, r29-css-v2 bf8eb71, r31-cover-cap-v2 3e86fef, r38-recovery-v2 6b36fdb (uses error kinds); all together: parked-all-v2 a87f361 (Shift+F10 fixed for WebKit); r15 not refreshed]
43. r41-failmemo: analysis-failures.json side file so broken files stop retrying every launch (no schema) [landed; skip after 2 unchanged failures, explicit request retries; app version bump retries all]
44. r42-realsoak: 45-60 min soak of the real Linux app (Tauri + WebKitGTK + real IPC) under xvfb, RSS of app + web process [running]
45. r43-a11y-webkit: fix r17's Shift+F10 playlist menu in WebKit on r17-a11y-v2, re-green parked-all-v2 in WebKit (prep only) [done: sidebar Shift+F10/Menu key handled in app (WebKit/WKWebView has no native contextmenu for it); r17-a11y-v2 7a8fbe5, parked-all-v2 a87f361]
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

