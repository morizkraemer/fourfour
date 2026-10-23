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
### Q29 Track row mount + 30k paint cost (own prio: perf) [running] — branch q29-rows (from q28-waveform-perf)
### Q27 Repro tests for writer bugs [done, branch q27-writer-repro 450ea04; doc landed] — PDB export fails from 192 tracks (137 with 200-char titles); ANLZ colour detail wrong from 437 s (4 overflow sites, anlz.rs:258/314/364/404). Ignored repro tests on branch. docs/architecture/writer-bugs-2026-10.md
### Q21 Analysis review pass 1 (items 8, 2, 5) [done, branch q21-analysis 1fc254c (contains q15-grid); doc landed] — unified analyzer `ffa` (decode once, all stages, memory-capped pool): ~4.8 tracks/min on 8 workers, 0.56 GB/worker; Beat This! = 93% of CPU; grid 72.2% held, BPM ±0.1 88.8%; key stage now the cyanbox port 58.3% (n=199). Licence flag: cyanbox key templates fit on GiantSteps MTG (annotations unlicensed).
### Q26 Analysis review pass 2 (item 8) [done, branch q26-analysis cdc4f30 (contains q21); doc landed] — CPU 74→70 s/track, 5.3 tracks/min at 8 workers, tempo scan 1.1 s→0.19 s; native-rate decode, stratum removed; grid 72.2% / BPM 87.8% / key 58.3%; ONNX Runtime only 1.3–1.5× → not adopted. Lame3.100 mp3s are one frame late (fix needs dev check).
### Q30 Grid round 3 (item 5) [running] — branch q30-grid (from q26-analysis): Lame3.100 rule on dev, half-beat phase, octave BPM

## Run
status: running · since 2026-10-23 14:07 · cap 2 workers · autonomy tight · overnight, until Merlin says stop
targets (Merlin, direct): 1 fastest startup · 2 bulk import + analysis speed · 3 sync several USBs at once · 4 instant track load incl. waveform · 5 better beat grid · 6 Rekordbox library import works · 7 research other Rekordbox clones (incl. new "lite" one), reuse code if licence allows · 8 review the analysis pipeline twice for speed + quality · 10 audio engine: no crashes, fast, responsive · 11 UI lightning fast
plan: Q19 clone research (7) ∥ Q20 startup + UI speed (1, 11) → Q21 analysis review + bulk speed + grid (8, 2, 5) → Q22 audio engine (10) → Q23 track/waveform load (4) → Q24 Rekordbox import (6) → Q25 multi-USB sync (3) → second analysis review pass (8) → own priorities
rules: all code on branches (tight); docs/reports land. Items 3/6 touch the brother's lane (sync engine, master.db reader): branch only, flagged for him. Third-party code: licence-checked, inspected in an isolated dir, never executed blindly.

## Rundown
Since 10-15 nothing ran. Master holds the Rust analysis prototype (BPM/grid/key), benchmark docs, UI review and three sync mockup rounds. Open: Merlin's sync round 3 answers, brother's engine work, Q15/Q16 proposed.
