# fourfour - state

_Updated: 2026-11-03 by orchestrator on `dev`. Live state only, max ~80 lines. History → docs/journal.md, facts → memory._
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
29. UI headroom: analysis pauses while scrolling. Background QoS behind FF_ANALYSIS_QOS. rec: keep defaults, decide after Mac night - branch: ov-1028
30. Clean key model v2: Beatport 57.3%, within noise. rec: leave unlanded, ship CLEAN_FSL, accept ~56% - branch: b14-cleankey60 11784f5
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

Note: land the -v4 branches (33-38) one by one, or parked-all-v7 15fd864 at once.

### Needs Merlin's ears/eyes (laptop)
- Preview, master: https://dev.tail87069a.ts.net:4200 (`preview start fourfour app-review`); dies on OOM.
- Playback ear check: 8 steps in docs/architecture/audio-engine-2026-10.md (branch q22-audio).

### Brother's lane
- Windows risks (r20): eject, removable flag, case-sensitive extensions. See robustness-2026-11.md, Platforms 2026-11.
- Windows libcrypto (r65): vendored-openssl SQLCipher feature under cfg(windows), when a Windows release is planned.
- r12: two same-name tracks collide on one USB path (repro: pioneer-library/tests/robust_store.rs).
- r6: cold 10k import reads 2.7 GB of ID3 pictures; seek-past reader in pioneer-usb-writer/src/scanner.rs.
- Sync engine per docs/ui/usb-sync-spec.md. Clone facts: docs/research/clones-2026-10-23.md.
- Open: Q6 key source PDB vs OneLibrary; .2EX / PWV6-7 for CDJ-3000X; CDJ-3000 round-trip.

42. Phrase detection (r90): first Rust detector, off by default, other results byte-identical; only 57 Rekordbox phrase refs exist, F 0.48 at +-1 bar (8-bar grid baseline 0.45), mood off - rec: you run Rekordbox phrase analysis on 200+ testdata tracks and copy the library to dev, then train labels - branch: r90-phrase f62246d. Brother: writer sets analysedBits 105 (phrase) but writes no PSSI, needs a CDJ check

## Queue
### Q5 Unsigned macOS build [queued] [mac] - cargo tauri build via mac-queue. 30 min
### Q6 Key source PDB vs OneLibrary [handed to brother]
### Q8 Cross-platform CI [queued] [box] - build, test, check on 3 OSes; branch only; first run needs Merlin's OK. 45 min
### Q14 USB sync UI [waiting on Merlin] - round 3 mockups decide the rebuild; branch ui-sync-flow b510e70
### Q50 Speed [running as holiday run] [box] - brief in docs/state-archive-2026-10-28.md (2026-11-03 section)

### Q51 Analysis + speed push to 2026-11-16 [running] [box+mac] - Merlin 2026-11-03: max gains, incremental. Result changes park with evals; identical-result speed lands on ov-1028. Order (refill as results come):
- r89 Mac night 2 [running]; r90 phrase [parked, Waiting 42]; r91 key mode [dropped: no clean gain, doc on r91-key-mode 80f6a03]; r92 import cover skip [backed out: robustness test 13x slower, r92b fixing]; r94 smaller beat model [running]; r89 Mac night 2 [landed 030a2a4]; r93 chunk ceiling by runtime [green, landing]; r95 50k startup [running]
- next: key mode (oracle mode +35 dev); import skip APIC covers; 50k startup (lazy rows, payload); LUFS/gain; energy 1-10 (Rust, no Lexicon); auto cues from phrases; BPM >1 misses; zoom first open; WebKit row cost; long-track RSS. Genre dropped: no labels/model

## Run
status: running since 2026-11-03 15:10 (Merlin go: Mac night 2 r89 + 2-week push on analysis quality and speed to 2026-11-16; identical-result speed wins land on ov-1028, result changes park with evals) · runs until Merlin says stop · holiday to 2026-11-16 · cap 2 · seat only · no reviews, take recs · green work lands on ov-1028 (+ Mac install); never master, never push · Mac only on Merlin's go (last 2026-11-01 21:42)
targets: beta-ready on ov-1028, fastest at 10k+ (startup, analysis, track load, scroll); speed first, then beta
parked for Merlin: r18-errors-v4 (33), r17-a11y-v4 (34), r31-cover-cap-v4 (35), r29-css-v4 (36), r76-recovery-kind (37), r52-no-python-v3 (38); all at once: parked-all-v7 15fd864. Not landing: r15-reveal, r16-quant (dropped), r35-ci (15)
next Mac night (needs go): perf-night.sh --size 10k --expand-runs 5 on ov-1028; compare r15-reveal and r29-css-v4; 50k only with >= 40 GB free; chunk-budget sweep: scripts/mac/chunk-budget-sweep.sh --n 200
review: https://dev.tail87069a.ts.net:4220 (cards) · https://dev.tail87069a.ts.net:4210 (ov-1028, mock) · https://dev.tail87069a.ts.net:4211 (parked-all-v7, mock)
Mac: ov-1028 in /Applications/fourfour.app (12:21 WITA); dmg at ~/dev/build/fourfour-mac/build/mac/dmg-20261101-1221/
Hard gates: no push, no master merge, no deploy, no CDJ/USB writes, brother's lane read-only

## Rundown
Run 2026-10-30 to 11-03 on ov-1028 (checks and smoke green): Mac analysis 2-3x faster on CoreML, 50k startup 2.2 -> 1.35 s, Linux RSS after analysis 2.1 -> 0.4 GB, Mac 5 s freeze fixed.
Parked 2026-11-03 (parked-all-v7, :4211): schema v8, BPM search, keyboard rating/seek/reorder, recovery by error kind, stick busy guard. Dropped: grid drift refinements (r77).
Waiting on Merlin: land parked-all-v7, CI r63-ci-v3 (15), docs (39), key licence (4), bar-1 listen (16), bundle id (24), a11y taste calls (34). Next Mac night needs a go.
