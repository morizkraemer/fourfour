# fourfour - state

_Updated: 2026-11-04 by orchestrator on `dev`. Live state only, max ~80 lines. History → docs/journal.md, facts → memory._
_Archived detail (trimmed 2026-10-27, 2026-11-03 twice): docs/state-archive-2026-10-28.md_

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
4. [accepted 2026-10-31] Key licence: clean FSL model ships, Beatport 56.3% (below 60% bar). Needs attribution. rec: ship clean-fsl for beta - branch: ov-1028 (switch in cyankey.rs)
5. Old stash@{0} (waveform tweaks, prototyping only). rec: build on a branch for review - branch: none
6. `.claude/settings.local.json` allows `git push *`, bypassing the push ask. rec: remove that allow line (Merlin edits it) - branch: none
7. Push master (30 commits ahead). rec: push - branch: master
15. [refreshed 2026-11-02] CI for macOS, Windows, Linux; never run on GitHub. Windows lacks libcrypto DLL. rec: OK to push r63-ci-v3 for a first run - branch: r63-ci-v3 f0827da
16. [accepted 2026-11-01] Bar 1 rule: Rekordbox agreement 61.5% -> 79.3%. 5 regressions to listen to. rec: listen on the Mac, then land - branch: b15-bar1 e717de0
21. Q50 rten 0.27 bump: slower, logits drift. rec: drop - branch: q50-rten27
24. Beta .dmg bundle id com.pioneer.test-ui (Pioneer is a trademark). rec: pick a neutral id before public beta, migrate data once - branch: none
27. [built 2026-10-31, unlanded] onnxruntime+CoreML: no CPU gain on dev. rec: Mac test on a Mac night, drop if no gain - branch: r9-ort 383ae88
29. UI headroom: analysis pauses while scrolling. Background QoS behind FF_ANALYSIS_QOS. r108: analysis uses cores-2; all cores would be +13% (16 cores) to +24% (8 cores) throughput. rec: keep defaults, test cores-1 on a Mac night - branch: ov-1028 Mac night 3: cores-1 = cores-2 (20.5 trk/min) on the Mac -> keep default, nothing to decide.
30. Clean key model v2: Beatport 57.3%, within noise. rec: leave unlanded, ship CLEAN_FSL, accept ~56% - branch: b14-cleankey60 11784f5 r162 (2026-11-08): segment-trained student CLEAN_FSL_FMA_SEG: test 58.3% (shipped 56.3%), all-598 CV 60.9%, Rekordbox agreement 43.2 -> 44.1%, same runtime; gain inside noise; linear template at its clean ceiling - branch r162-key-accuracy 2aa0fb0 (includes b14). rec: take it in the next review build, ship after a listen-free sanity check; stop key model work.
31. [built 2026-11-03, schema v8] library.db 7.5 GB -> 59 MB at 50k. Open: fsync makes cover fill 2.5x slower. rec: keep durability - branch: r84-path-keys 05ebecd (includes r81)
33. [built 2026-11-03] Error messages (r18): 42 rewritten, 13 silent failures surfaced; analysis and playlist saves hit the browser mock. rec: land first after holiday - branch: r18-errors-v4 ad6ed3f. Open: recovery screen for open panic; status bar vs progress (toast?)
34. [built 2026-11-03] Keyboard + screen reader (r17/r78/r82). rec: land after a Mac WebKit check. Taste calls: rating column default, waveform ring width, F6 through search - branch: r82-r78-fix 788db39
35. Cover cache cap (r31). rec: land after a quick look (no blank rail, inspector or player) - branch: r31-cover-cap-v4 39ed323
36. WebKit startup (r29): row style -32 to -40%, pixel-identical. Edits AGENTS fence. rec: land both steps together - branch: r29-css-v4 2c37766 (or r46-dsrows-v3 63c5b94)
37. Library recovery panel (r38): shows why library.db will not open, plus retry/restore/new. rec: land after review, say "the app" not "fourfour" - branch: r76-recovery-kind b4171e5
38. No Python in beta (r52): Python fallback removed, engine picker gone. rec: land - branch: r52-no-python-v3 2933305
39. Beta user docs draft (r53): 20 "(to confirm)" marks need a Mac. rec: read on the phone, confirm marks on a Mac night - branch: r53-user-docs-v2 56f7625
40. Stick busy guard (r69): one stick operation at a time, sync off main thread. rec: land - branch: r69-stick-busy-v2 9a928da
41. [r88 2026-11-03, grid at ceiling] BPM fold-peak search on by default: test 180 -> 186/205. rec: land (~3% of tracks change, toward Rekordbox) - branch: r79-bpm-misses ce1d8b1
43. [accepted 2026-11-07 by Merlin: ship, Mac at 500 too] [built 2026-11-03] Beat chunk 500 default (r96+r105+r108): CPU/track -43%, no sys-time waste (r106), 2.1x on 8 GB machines; test grid 171=171, BPM 0.1 185 vs 186, zero right->wrong, dev 44 -> 43. Mac app keeps 1500. rec: land; Mac at 500 is your call - branch: r108-throughput 1ee34d2 (on r105-snap 903b702) Mac night 3 (2026-11-06): CoreML chunk 500 vs 1500: 132.9 vs 102.6 trk/min (+30%), RAM 6.5 -> 2.6 GB, first result 11.4 -> 6.6 s. rec: land, Mac at 500 too.
Note: everything parked is in parked-all-v19 a180278 = build 1404 (v18 + r166 fling text (incl r164 row cost) + r163 cold open + r162 key model), preview :4212, installed on the Mac 2026-11-08 13:59 WITA (all checks green). v18 had: v17 + ov-1028 with r156-r158 (Mac analysis: chunk 500 + int8 + prepack, model bundled) + r159 scroll edges + r160 cues/grid/playhead. Lab cards open: r159-scroll-edges, r160-cues-grid. 1/16 snap for all cues [accepted 2026-11-08 per rec].
44. [accepted 2026-11-07 by Merlin: ship int8 + prepack on Mac] [built 2026-11-04, Mac-checked] int8 beat model at chunk 500 for the Mac (r97/r98): same Mac session vs CoreML 1500: +22% trk/min, RAM 6.9 -> 2.4 GB, first result 6.7 -> 4.2 s, scores equal within 1 track; but 2.7x CPU (about 11 cores vs 4: fans, battery). x86 keeps fp32. rec: ship on Mac with fewer analysis workers on battery - branch: r98-mac-pair 87548eb (on r97-int8-500) Mac night 3: int8 at 500 177.0 trk/min (+72% vs CoreML 1500), RAM 2.3 GB, first result 4.7 s, CPU 614 vs 644 s (no longer higher). r150: rten weight prepack for int8 = -4.4% CPU, byte-identical (fp32 no gain) - branch r150-int8-prepack e779648, add to the next Mac night.
45. [built 2026-11-04] Loudness (r99): Rust LUFS + true peak, within 0.013 LU of ffmpeg on 22 tracks, ~1% CPU, off by default, outputs byte-identical. Showing it needs a library.db column (schema) and a writer field (brother). Reference -14 LUFS gives -1.5 to -8.8 dB on club tracks; reference level is your call. rec: land the analysis part, decide reference + display later - branch: r99-lufs cae9320
46. [built 2026-11-04] Remaining BPM misses (r101, on r79+r96): 12 test misses classified; only octave errors are rule-fixable. A half-time band (84-92.5 BPM doubled) fixes 2, no flips, test BPM 1 193 -> 195. Fitted to 2 tracks and wrong for hip-hop/downtempo. rec: ship it off; later a Rekordbox-style BPM range setting (your UI call). Also: chunk 500 costs 3 BPM results vs r79 alone (test 186 -> 183) - branch: r101-bpm-misses 11d94ba

42. Phrase detection (r90): Rust detector, off by default; F 0.48 vs 57 Rekordbox refs (baseline 0.45). rec: run Rekordbox phrase analysis on 200+ testdata tracks and copy the library to dev - branch: r90-phrase f62246d. Brother: writer sets phrase bit but writes no PSSI
47. [built 2026-11-04] Energy 1-10 (r100): Rust, off by default, outputs byte-identical, <1% CPU, stable across re-encodes. No real labels exist; vs a genre proxy Spearman 0.48 (ends right, middle noisy), ratings skew low on your library. rec: rate ~100 tracks 1-10 on the phone (or share any MIK/Lexicon ratings) so it can be calibrated before it shows - branch: r100-energy 9c9f226
48. [accepted 2026-11-07 by Merlin: fence edit OK] [built 2026-11-04] Playhead without per-frame relayout (r112): drive the waveform playhead with a paused Web Animation instead of style.transform; pixel-identical (72 snapshots), Layerize 658 -> 27 ms per 1.5 s while a track plays with the player open. File is in prototyping/ (AGENTS fence). r123: while a track plays it halves Chromium CPU (compact 1045 -> 528 ms/s, expanded 1258 -> 838). r118 confirmed: rows-as-images (the other route) is not pixel-identical and grows memory, so this is the only clean compositor fix. rec: OK the fence edit - branch: none, diff in perf-run-2026-11.md R112

49. [built 2026-11-04] Key 60% bar (r116): misses are 74% mode/neighbour (parallel, fifth, relative); 20-45% look like label noise (annotator unsure, key changes). Linear model on this chroma is saturated (refit on Beatport: 58.7%). Best switch clean-fsl-harmonic: 59.9% (test 56.3 -> 59.8) but changes 16% of keys, gain not significant (p~0.3). rec: keep clean-fsl; treat 60% as inside label noise. r117: beat-synchronous, chord and Tonnetz features add nothing (best test 56.8%); next lever is clean full-track training data - branch: r116-key-misses 53c9e39 (r117-key-beatsync ddacd15)

50. [built 2026-11-05] Idle/playing backend wakeups (r123): playback decoder poll 0.5 -> 50 ms and tick 1/s when the window is hidden (backend wakeups while playing 1580 -> 39 per s, CPU 64 -> 17 ms/s); volume poll paused when hidden and diskutil cached. Screen output identical; audio path untested on a real device. rec: listen on the Mac (long track, small buffer, seek) then land - branch: r123-idle-cpu 491ffcb
51. [built 2026-11-07] Waveform Meter::push is 44% of non-model analysis instructions; R104 estimates 4x on that stage (~0.7 s/track) with unchanged float ops. File is in pioneer-usb-writer/src (brother lane). rec: hand to your brother as a speed ask - branch: none, see perf-run-2026-11.md R127
52. [built 2026-11-07] Real-app Mac soak (r140) needs the Mac unlocked: a locked screen stops WebKit frames, so only idle was measured (10k: app 214 MB, WebKit 160 MB, 0% CPU). rec: leave the Mac unlocked on power one evening, I run scripts/mac/r140-soak.sh (~1 h) - branch: r140-mac-soak 9d53fae
53. [built 2026-11-08] Settings sheet leak (r169): a closed settings sheet stayed in memory (151 nodes) after Escape, via a Svelte delegated-event getter that V8 caches; fix = native keydown listener in SettingsSheet.svelte (prototyping, AGENTS fence), deterministic test added; same pattern exists in CommandPalette, DetailPane, CueEditor (bounded, not fixed). rec: OK the fence edit and land - branch: r169-settings-leak e5bc972

### Needs Merlin's ears/eyes (laptop)
- Preview, master: https://dev.tail87069a.ts.net:4200 (`preview start fourfour app-review`); dies on OOM.
- Playback ear check: 8 steps in docs/architecture/audio-engine-2026-10.md (branch q22-audio).

### Brother's lane
- Windows risks (r20): eject, removable flag, case-sensitive extensions. See robustness-2026-11.md, Platforms 2026-11.
- Windows libcrypto (r65): vendored-openssl SQLCipher feature under cfg(windows), when a Windows release is planned.
- r12: two same-name tracks collide on one USB path (repro: pioneer-library/tests/robust_store.rs).
- r104: waveform Meter::push (pioneer-usb-writer) is the biggest non-model stage, 740 ms/track at 53 ns/sample vs 8.5 ns for a plain biquad; mono fast path + state in locals could be 4x, bit-identical.
- r6: cold 10k import reads 2.7 GB of ID3 pictures; seek-past reader in pioneer-usb-writer/src/scanner.rs.
- Sync engine per docs/ui/usb-sync-spec.md. Clone facts: docs/research/clones-2026-10-23.md.
- Open: Q6 key source PDB vs OneLibrary; .2EX / PWV6-7 for CDJ-3000X; CDJ-3000 round-trip.


## Queue
### Q5 Unsigned macOS build [queued] [mac] - cargo tauri build via mac-queue. 30 min
### Q6 Key source PDB vs OneLibrary [handed to brother]
### Q8 Cross-platform CI [queued] [box] - build, test, check on 3 OSes; branch only; first run needs Merlin's OK. 45 min
### Q14 USB sync UI [waiting on Merlin] - round 3 mockups decide the rebuild; branch ui-sync-flow b510e70
### Q50 Speed [running as holiday run] [box] - brief in docs/state-archive-2026-10-28.md (2026-11-03 section)

### Q51 Analysis + speed push to 2026-11-16 [running] [box+mac] - Merlin 2026-11-03: max gains, incremental. Result changes park with evals; identical-result speed lands on ov-1028. Order (refill as results come): r124 playlists landed d77ce38 (20k switch 285 -> 82 ms, add 500 40 s -> 57 ms). r126 rescan landed 8dbf8f0 (50k no-change 1438 -> 723 ms). r127 decode: no measurable gain, branch kept r127-decode 3a4d21f. r129 budget doc merged (5 levers). Paused for Merlin feedback: r128-track-load 222eee8, r130-bulk-bump 2ffc198 (validation pending). Running: r131 import flow, r132 player polish (feedback on build 1282).
- landed on ov-1028: r89, r93, r95, r92b, r104, r106 (doc), r109 early load_state, r111 load_state 2x, r112 (harness) eb9d84f. Parked: 42-48. Dropped: r91 key mode, small models, r107 grid misses, r110 first-result burst (branch kept). r113 search: 50k sorted clear 182 -> 74 ms, sort click 172 -> 137, identical order; landed 1d9fdb1. r114 bundle: entry JS 512 -> 371 kB, 10k first row -25 to -126 ms, first screen byte-identical; landed 1f9e6c8; Mac check pending. Parked: r116 key misses (49). r115 first sort: 50k title 228 -> 87 ms, artist 186 -> 89, identical order; landed 0e01eba. Dropped: r117 beat-sync key. Dropped: r118 rows-as-images (spec + doc merged 13dee70). Built: parked-all-v9 3a135a7 (r119). r120 binary load_state: 50k page decode 214 -> 126 ms (WebKit 182 -> 71), first row 50k -118 to -282 ms, identical rows; landed 7056104; real Tauri IPC path needs a Mac check. r121 mount: skeleton rows as HTML, closed menus mount on open; mount CPU -21%, 10k first row about -50 ms; landed a7500d8. Parked: r122 lazy rows (ef8dbdc: 50k first row -50 to -140 ms but +170 ms first-keystroke hitch and more idle CPU; not worth the complexity). Parked: r123 idle CPU (50, needs Mac ears). Built: Mac night 3 runner (r125, e51d9e7). Running: r124 playlist views (opus continuation after the sonnet weekly limit; resets 17:00 UTC). History: journal + archive
- next: auto cues from phrases; zoom first open (mostly done R25-R57); parked-all-v7 refresh (libc fix). Genre dropped: no labels/model

## Run
status: running since 2026-11-03 15:10 (Merlin go: Mac night 2 r89 + 2-week push on analysis quality and speed to 2026-11-16; identical-result speed wins land on ov-1028, result changes park with evals) · runs until Merlin says stop · holiday to 2026-11-16 · cap 2 · seat only · no reviews, take recs · green work lands on ov-1028 (+ Mac install); never master, never push · Mac only on Merlin's go (last 2026-11-01 21:42)
targets: beta-ready on ov-1028, fastest at 10k+ (startup, analysis, track load, scroll); speed first, then beta
parked for Merlin: r18-errors-v4 (33), r17-a11y-v4 (34), r31-cover-cap-v4 (35), r29-css-v4 (36), r76-recovery-kind (37), r52-no-python-v3 (38); all at once: parked-all-v7 15fd864. Not landing: r15-reveal, r16-quant (dropped), r35-ci (15)
next Mac night (GO from Merlin 2026-11-06, Mac on ~1 h later, free use): scripts/mac/night3.sh --remote from the ov-1028 worktree (q41-int; ~2.5 h, 35 GB free): app open + perf night + r110 quiet run + chunk 500/int8 sweep + cores split, summary back on dev. Ears: r123 audio (own install), bar-1 (16), docs marks (39). mac-queue has no job kind for it, so it uses ssh mbp like ship-dmg.sh
review: https://dev.tail87069a.ts.net:4220 (cards) · https://dev.tail87069a.ts.net:4210 (ov-1028, mock) · https://dev.tail87069a.ts.net:4212 (parked-all-v9, mock; worktree agent-a5897e946a999da28 kept for it, build in ~/.cache/preview/fourfour-parked-v9)
Mac: ov-1028 in /Applications/fourfour.app (12:21 WITA); dmg at ~/dev/build/fourfour-mac/build/mac/dmg-20261101-1221/
Hard gates: no push, no master merge, no deploy, no CDJ/USB writes, brother's lane read-only

## Rundown
Run 2026-10-30 to 11-04 on ov-1028 (all checks green): Mac analysis 2-3x faster, 50k startup 2.2 -> 1.35 s, load_state 2x faster, import end to end 1.9 s at 10k, key -60% and decode -13% CPU.
Parked for Merlin: parked-all-v9 (:4212), chunk 500 (43), Mac int8 (44), loudness (45), BPM range (46), energy labels (47), playhead fence edit (48). Next Mac night needs a go.
