# fourfour - state

_Updated: 2026-11-22 by orchestrator on `dev`. Live state only, max ~80 lines. History → docs/journal.md, facts → memory._
_Archived detail (trimmed 2026-10-27, 2026-11-03 twice): docs/state-archive-2026-10-28.md_

## Direction
Not in Compass (baseline 2026-09-24: creative block to 30 Oct, then Pad, then Bounce). Quiet engineering only; never takes Merlin's focus hours. Holiday ended 2026-11-13.
Roles (2026-11-22): this thread now also owns the USB writer and sync engine; Mo (GitHub morizkraemer) does real CDJ/hardware testing only.
Licence (2026-11-22, Merlin + Mo): GPL-3.0-or-later (LICENSE in master 7becfea); rbxport (GPL) code may be reused. Not for sale, fully open source. Merlin travels: no hardware.
Analysis: Rust-native (Rust DSP + Beat This! ONNX), permissive licences only, no Python/AGPL. Key: shipped clean FSL model, 78.9% exact on held-out GiantSteps+ (R177); key model work stopped.
USB sync: only chosen playlists go on a stick; plug in = sync; ask only on removals / won't fit / unanalyzed; never delete files fourfour didn't write. Design log: docs/ui/usb-sync-decisions.md; engine spec for the brother: docs/ui/usb-sync-spec.md.

## Ship target
Public macOS beta: a .dmg that scans a folder, analyzes it in Rust and exports a USB that plays on a CDJ-3000. Done when: the sync UI round 3 is built and reviewed (Q14), the brother's engine supports playlist-scoped sync, the bundle id is neutral (24), and the app is signed.

## Rulebook
AGENTS.md ## Unattended (autonomy tight). No Tauri windows, audio output or USB writes on dev; those are `[mac]`. Workers: API credit runs with per-run --budget; shared cap $450/month all projects.

## Branches and builds
- master d231066 (+ docs) = review build 1427 code (parked-all-v23 merged), pushed 2026-11-15 per Merlin. Build 1974 from master.
- Draft GitHub pre-releases (repo is public; drafts visible to collaborators only): v0.1.0-alpha.4 = build 1999 (for Mo), alpha.3 = build 1996, alpha.2 = build 1974, alpha.1 = build 1425.
- Mac: build 1427 in /Applications. ov-1028 is superseded by master (its 7 extra commits: merge records + r177 truth-set scripts, no app change).

## Waiting on Merlin
5. Old stash@{0} (waveform tweaks, prototyping only). rec: build on a branch for review - branch: none
6. `.claude/settings.local.json` allows `git push *`, bypassing the push ask. rec: remove that allow line (Merlin edits it) - branch: none
15. CI for macOS, Windows, Linux; never run on GitHub. Windows lacks libcrypto DLL. rec: OK to push r63-ci-v3 for a first run - branch: r63-ci-v3 f0827da
16. Bar 1 rule (landed, in master): 5 regressions never listened to. rec: listen on the Mac; revert = BAR_ONE_RULE ModelVote - branch: master
21. Q50 rten 0.27 bump: slower, logits drift. rec: drop - branch: q50-rten27
24. Bundle id com.pioneer.test-ui (Pioneer is a trademark). rec: pick a neutral id before public beta, migrate data once - branch: none
39. Beta user docs draft (r53): 20 "(to confirm)" marks need a Mac. rec: read on the phone, confirm marks on a Mac - branch: r53-user-docs-v2 56f7625
42. Phrase detection (r90): Rust detector, off by default; F 0.48 vs 57 Rekordbox refs. rec: run Rekordbox phrase analysis on 200+ testdata tracks and copy the library to dev - branch: r90-phrase f62246d
45. Loudness (landed in master, off by default): reference level and display are your call (-14 LUFS gives -1.5 to -8.8 dB on club tracks). rec: decide later with a library.db column - branch: master
46. BPM half-time band (landed, off): later a Rekordbox-style BPM range setting (UI call). rec: later - branch: master
47. Energy 1-10 (landed, off by default): no real labels; DEAM unreachable from dev. rec: rate ~100 tracks 1-10 on the phone, or let me fetch DEAM from another network - branch: master
50. Idle/playing backend wakeups (r123): wakeups while playing 1580 -> 39/s; audio untested on a device. rec: listen on the Mac (long track, small buffer, seek), then land - branch: r123-idle-cpu 491ffcb
52. Real-app Mac soak (r140) needs the Mac unlocked on power ~1 h. rec: one evening, I run scripts/mac/r140-soak.sh - branch: r140-mac-soak 9d53fae
54. Focus in other views (r174), review only, not in master. rec: put it in the next review build - branch: r174-focus-views
55. Q14 USB sync UI round 3 mockups decide the rebuild. rec: review the mockups - branch: ui-sync-flow b510e70

### Needs Merlin's ears/eyes (laptop)
- Playback ear check: 8 steps in docs/architecture/audio-engine-2026-10.md.
- Ears: bar-1 (16), r123 audio (50), docs marks (39).

### Brother's lane
- Windows risks (r20): eject, removable flag, case-sensitive extensions. See robustness-2026-11.md, Platforms 2026-11.
- Windows libcrypto (r65): vendored-openssl SQLCipher feature under cfg(windows), when a Windows release is planned.
- r12: two same-name tracks collide on one USB path (repro: pioneer-library/tests/robust_store.rs).
- r104/R127: waveform Meter::push (pioneer-usb-writer) is the biggest non-model stage (~740 ms/track); mono fast path could be 4x, bit-identical.
- r6: cold 10k import reads 2.7 GB of ID3 pictures; seek-past reader in pioneer-usb-writer/src/scanner.rs.
- Phrase: writer sets the phrase bit but writes no PSSI.
- Sync engine per docs/ui/usb-sync-spec.md. Open: Q6 key source PDB vs OneLibrary; .2EX / PWV6-7 for CDJ-3000X; CDJ-3000 round-trip.

## Queue
### Q63 Writer v2 [running] [box] - plan docs/plans/writer-v2-2026-11-22.md (Fable): vendor rbxport rbl-pdb + rbl-anlz behind a stick model, keep our OneLibrary writer/meter/multi_sync; byte diff vs a small Rekordbox export (FF-GOLDEN, Merlin exports A/A2/B); old writer stays default until diff + Mo's CDJ-3000 check. Slices 1-3 running on branch writer-v2. Fixes #41 (Rekordbox stick library wiped on first sync), #42
### Q64 Stick UI round 1 [running] [box] - Merlin's Mac feedback 2026-11-22: status dot (not selection), Keep updated toggle bug, stick info into the inspector, side panel = plain dual-window list, no "in sync" pill when fine, stick track row actions. Fable, branch stick-ui-r1, lab card
### Q65 Play from stick + Tab [running] [box] - play tracks from a stick, Tab with no track loaded, plan for opening Rekordbox sticks + importing playlists (on Q63's stick model). Branch stick-play-tab
### Q6 Key source PDB vs OneLibrary [folded into Q63]
### Q8 Cross-platform CI [waiting on Merlin, 15] [box] - build, test, check on 3 OSes; branch only. 45 min
### Q14 USB sync UI [waiting on Merlin, 55] - branch ui-sync-flow b510e70
### Q60 Duplicate cleanup [queued] [box] - keeper + archive + restore, proposals you accept (Ora-style), reuse dup data and the 2 s duration rule; UI on a branch, lands only after Merlin's review. Research: docs/research/ora-docs-2026-11.md
### Q61 Moved-file reconnect + folder relocate [queued] [box] - reconnect by audio fingerprint with copy-vs-move and two-candidates guards; relocate a folder. Branch only
### Q62 Playlist export [queued] [box] - M3U8, CSV, Traktor NML, Rekordbox XML (lists only). Branch only
### Mo's issues (2026-11-20): #35 compatible copies, #36 staged sync + verify + recover, #37 import back from stick, #38 writer audit vs rbxport gotchas
### Q5 Unsigned macOS build [landed] - DMG pipeline works (scripts/mac/build-dmg.sh, check-dmg.sh); signing still open.

## Run
status: stopped · the 2-week analysis + speed push (Q50/Q51, 2026-11-03 to 2026-11-16) ended; results in master. No workers running. Next run only on Merlin's go.
review: https://dev.tail87069a.ts.net:4220 (lab cards, all reviewed and cleared 2026-11-13)
Hard gates: no push to master without Merlin's OK, no deploy, no CDJ/USB writes, brother's lane read-only

## Rundown
Push 2026-10-30 to 11-16, all in master: Mac analysis about 3x faster (int8 + chunk 500: 351 trk/min, 2.3 GB), 50k startup 2.2 -> 1.35 s, import index 713 -> 153 ms, scroll/fling fixes, hot cue grid + declick, inspector close/auto-open, keys 1-8 fire cues, overlay leak fixes, error messages, recovery panel, keyboard/screen reader.
Truth sets (R177): key 78.9% exact held out, BPM Acc1 92.4%.
Released: draft v0.1.0-alpha.2 (build 1974) for Mo.
Next: Merlin's picks from Waiting (bundle id 24 and sync UI 55 block the public beta).
