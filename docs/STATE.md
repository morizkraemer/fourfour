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
0. USB export fails from 192 tracks; colour waveforms wrong on tracks > 7:17 (brother's writer) — rec: send your brother docs/architecture/writer-bugs-2026-10.md first thing — branch: q27-writer-repro
1. Sync round 3 (https://claude.ai/artifact/1cCJTk3r92jC6kmJmz1nnJ): panel closed → whole stick view as drop target, or a "Put on stick" strip? — rec: strip — branch: none
2. Copy a playlist to another stick: Keep updated on by default? — rec: yes — branch: none
3. Build round 3 into the app (replaces ui-sync-flow's first attempt, keeps the stick header)? — rec: yes after 1–2 — branch: ui-sync-flow
4. Key: shipped key stage is now the cyanbox port (BSD-2 code) whose templates were fit on GiantSteps MTG (annotations unlicensed); keydet weights fit on CC BY-SA. Both need your licence call — rec: refit templates on permissively licensed labels before shipping. Was: key detector weights fit on CC BY-SA 4.0 data — rec: ship only weights trained on permissively licensed data — branch: none
5. Old stash@{0} (scrub-pause waveform tweaks) — rec: build on a branch for review — branch: none
6. `.claude/settings.local.json` allows `git push *`, which bypasses the push ask rule — rec: remove that allow line (Merlin edits it) — branch: none
8. [accepted 2026-10-23 per rec] Add vitest as an app dev dependency for unit tests (Q18, branch only) — rec: keep — branch: q18-app-tests
7. Push master (30 commits ahead, docs + analysis prototype + tests) — rec: push — branch: master

### Needs Merlin's ears/eyes (laptop)
- Preview of tonight's combined branch (q31-tests; q32-search adds the search fix): https://dev.tail87069a.ts.net:4250 (?mock=30000 for a big library)
- Preview, master: https://dev.tail87069a.ts.net:4200 (`preview start fourfour app-review`); dies on OOM, restart by hand.
- Playback ear check: 8 steps in docs/architecture/audio-engine-2026-10.md (branch q22-audio).

### Brother's lane
- **CRITICAL (Q25): `pdb.rs::split_into_pages` panics at ~200+ tracks (u16 heap offsets wrap past 64 KiB) → write_usb/sync_usb fail on any real library. Fix: usize offsets, rebase per chunk.**
- Bug from Q24: writer/anlz.rs colour-detail index `i * src_len / entry_count` overflows u32 on tracks > ~7 min (panics in debug). Also: parse_pco2 drops cue names/colours; SQLCipher keys private; no PDB reader (docs/architecture/rekordbox-import.md).
- From clone research: Booth 0.0.9 CDJ-3000 fw 3.22 fixes, baken waveform/PVB2/PSSI (MIT), cyanbox MP3 timeline rule + PWAV min heights — see docs/research/clones-2026-10-23.md "Format facts".
- Sync engine changes per docs/ui/usb-sync-spec.md (playlist scope, ownership manifest, dry-run plan, progress events, capacity, removable-drive filter, wipe guard).
- `get_mounted_volumes` lists the system disk ("Macintosh HD") and offers Wipe: must fix.
- Q6 key source PDB vs OneLibrary; .2EX / PWV6-7 placement for CDJ-3000X; CDJ-3000 round-trip of a current export.

## Queue
### MERGED 2026-10-26 into master: q40-rb-match (whole app stack + waveforms) and q30-grid (analysis prototype). Full checks green (Rust tests, 172 app tests, check, build). CDJ checks for the brother: docs/architecture/cdj-checklist-2026-10.md.
### Q1–Q4, Q7, Q9–Q13 [landed] — playback, svelte-check 0, export tests, browser preview, benchmark, cache fix, packaging decision, key detector (57%), BPM/grid (BPM ±0.1 86%, grid ±20 ms 47%). See journal + docs/analysis/.
### Q5 Unsigned macOS build [queued] [mac] — `cargo tauri build` via mac-queue; report artifact + log; no signing. 30 min
### Q6 Key source PDB vs OneLibrary [handed to brother]
### Q8 Cross-platform CI [queued] [box] — Actions on macOS/Windows/Linux (build, test, check, unsigned tauri build). Branch only; first run on the shared repo needs Merlin's OK. 45 min
### Q14 USB sync UI [waiting on Merlin] — first attempt on ui-sync-flow (b510e70, preview :4240 when started); round 3 mockups decide the rebuild.
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
### Q33 Wire the Rust analyzer into the app (item 2) [paused, WIP] — branch q33-rust-analysis: lib crate fourfour-analysis-rs extracted (c3cb7a2); backend/coordinator files WIP-committed unbuilt; worktree .claude/worktrees/agent-a45ef63857b3fe37b

## Run
status: idle · waveforms accepted "good enough for now" by Merlin 2026-10-25 · cap 2 · autonomy tight
targets: 3-band colour waveforms in app + USB export — done on branch q37-waveforms (contains q34 export, q35 app, q36 integration, q37 polish; based on the q32-search app stack)
rundown: app and stick draw the same 3-band bytes (low blue front, mid amber, high white); 7:17 colour bug fixed (tests to 60 min); vs Rekordbox's own bytes on held-out tracks corr low .96 / mid .98 / high .93, PWV3 heights 99.7% exact; perf within noise of q32-search, startup 30k 0.35 s.
screenshots: review page https://claude.ai/artifact/7XmvdpXferK7rfBqLrEKxU · before /tmp/claude-1000/-home-merlin-dev-projects-fourfour/5e0c999f-66b8-4690-ac43-edb78b690bb0/scratchpad/waveforms/app/before/ · after /tmp/claude-1000/-home-merlin-dev-projects-fourfour/5e0c999f-66b8-4690-ac43-edb78b690bb0/scratchpad/waveforms/app/after/ and app/after-v2/ · side-by-side /tmp/claude-1000/-home-merlin-dev-projects-fourfour/5e0c999f-66b8-4690-ac43-edb78b690bb0/scratchpad/waveforms/app/compare/ · ours vs Rekordbox /tmp/claude-1000/-home-merlin-dev-projects-fourfour/5e0c999f-66b8-4690-ac43-edb78b690bb0/scratchpad/waveforms/final-v2/ (v1: final/) · export PNGs /tmp/claude-1000/-home-merlin-dev-projects-fourfour/5e0c999f-66b8-4690-ac43-edb78b690bb0/scratchpad/waveforms/export/
held for Merlin: visual sign-off; schema v4 (additive row_preview column); USB output changes (new .2EX, PWV5 real colour, measured PWVC gains) need the brother's CDJ checks (CDJ-3000, 3000X, older); Q33 stays paused.

## Rundown
Overnight: startup 30k 24.7 s→0.5 s backend / 188 s→3.3 s first paint; track load ~300→50 ms; fling CPU −55%; search first key 445→120 ms; audio engine 11 fixes + 26 tests; Rekordbox import (master.db/USB/XML); multi-USB sync 2.4–3×; grid 47→78.5%; analysis CPU 74→70 s/track; 108 app tests. All on branches (tight); docs on master.
Critical for the brother: USB export fails from 192 tracks; colour waveform wrong > 7:17 (docs/architecture/writer-bugs-2026-10.md).
