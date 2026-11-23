# fourfour - state

_Updated: 2026-11-23 by orchestrator on `dev`. Live state only, max ~80 lines. History → docs/journal.md, facts → memory._
_Archived detail: docs/state-archive-2026-10-28.md_

## Direction
Not in Compass. Quiet engineering; never takes Merlin's focus hours.
Roles (2026-11-22): this thread owns the USB writer and sync engine too; Mo (GitHub morizkraemer) does real CDJ/hardware testing only.
Licence (2026-11-22, Merlin + Mo): GPL-3.0-or-later; rbxport (GPL) code may be reused. Not for sale, fully open source.
Analysis: Rust-native (Rust DSP + Beat This! ONNX), permissive model licences only. Key 58% exact vs rekordbox 51% (Beatport test split); BPM on par; 512-track folder 6:44 vs rekordbox 34:19.
USB sync: only chosen playlists go on a stick; plug in = sync; ask only on removals / won't fit / unanalyzed; never delete files fourfour didn't write. Design log: docs/ui/usb-sync-decisions.md.

## Ship target
Public macOS beta: a .dmg that scans a folder, analyzes it in Rust and exports a USB that plays on a CDJ-3000. Done when: the stick flow is reviewed (Q68), writer v2 passes the byte diff and Mo's CDJ-3000 check (Q63), the bundle id is neutral (24), and the app is signed.

## Rulebook
AGENTS.md ## Unattended (autonomy tight). No Tauri windows, audio output or USB writes on dev; those are `[mac]`. Workers: API credit runs with per-run --budget; shared cap per month across projects. Master push only with Merlin's OK per push.

## Branches and builds
- master f324215+ (pushed 2026-11-23) = rc 932dcc3 merged: stick UI r1-r2, play from stick, Tab fix, duplicates, reconnect, playlist export, writer v2 (off, FOURFOUR_SYNC_ENGINE=v2), perf. A few docs commits unpushed.
- Mac: build 2148 (= master app code) in /Applications, bundle id app.ffdj.library. Blog post published 2026-11-22.
- Analysis branches (not merged, all off): phrase-r2 4cc1ab9, truth-eval 19f31bf, genre-r1 60cd1f9, h2h-rekordbox d591713, mac-2025-run 2171b43.

## Waiting on Merlin
1. Stick UI round 3 (Q68, running): review the lab card when it lands - branch: stick-ui-r3
2. FF-GOLDEN: small Rekordbox export on INTENSO (5 tracks; exports A, A2, B, copied off between each). Gates the writer v2 byte diff - branch: writer-v2 (in master)
3. Mo's CDJ-3000 test of writer v2: steps still to write. rec: I write them, you OK filing them as an issue - branch: master
4. Security review (public repo, report ~/dev/pm/reviews/2026-11-23-security-fourfour.md; no fixes run, ranked):
   a. Corpus manifests with real track paths: untracked + pushed 2026-11-23 per Merlin; still in history. rec: leave history unless the list matters (rewrite = Mo force-push)
   b. AGENTS.md, STATE, journal, state-archive, beta checklist are public (hostnames, notes, budgets). rec: move orchestrator docs to a private repo
   c. Lab card media (33 png/mp4) not checked for personal library content. rec: a worker views and lists any to replace
   d. /Users/merlinkraemer and /home/merlin paths in tests, scripts, docs. rec: neutral paths over time
   e. Draft DMGs may embed build paths. rec: check before publishing any release
6. `.claude/settings.local.json` allows `git push *`, bypassing the push ask. rec: remove that allow line (Merlin edits it)
15. CI on 3 OSes never run on GitHub. rec: OK to push r63-ci-v3 for a first run - branch: r63-ci-v3 f0827da
16. Bar 1 rule (in master): 5 regressions never listened to. rec: listen on the Mac - branch: master
21. Q50 rten 0.27 bump: slower, logits drift. rec: drop - branch: q50-rten27
24. Bundle id final name. rec: pick before public beta - branch: none
39. Beta user docs draft: 20 "(to confirm)" marks. rec: confirm on a Mac - branch: r53-user-docs-v2 56f7625
42. Phrase detection r2: labels 0.555 -> 0.605, boundaries flat (F 0.49). rec: keep off - branch: phrase-r2 4cc1ab9
45. Loudness reference level (off by default). rec: decide later
47. Energy 1-10 (off): vs DEAM Spearman 0.58 (electronic 0.65). rec: keep off until a UI need - branch: truth-eval
50. Playback wakeups (r123) untested on a device. rec: listen on the Mac, then land - branch: r123-idle-cpu 491ffcb
52. Real-app Mac soak (r140) needs the Mac ~1 h on power. rec: one evening - branch: r140-mac-soak 9d53fae
5. Old stash@{0} (waveform tweaks, prototyping only). rec: drop or branch - branch: none

### Needs Merlin at the Mac
- Ears: bar-1 (16), r123 audio (50), playback ear check (docs/architecture/audio-engine-2026-10.md).
- Manual tests of build 2148 done 2026-11-23; feedback went into Q68.

## Queue
### Q66 Genre r2: beat-model features [running] [box] - pooled Beat This hidden states as genre features; opus, $10 cap; branch genre-r2-beatfeat
### Q67 Genre r2: free training data [running] [box] - Jamendo/Freesound/netlabel CC BY/CC0 club audio, retrain, score; sonnet, $10 cap; branch genre-r2-data
### Q68 Stick UI round 3 [running] [box] - Fable, $15 cap: toggle shift, false "changes waiting", sidebar dot, click focus ring; flow review (keep updated, pinned sticks, inspector vs panel, top bar, menu labels); drag stick tracks into library (copies audio). Lab card stick-ui-r3
### Q63 Writer v2 [in master, off] - next: FF-GOLDEN byte diff (slice 4), then Mo's CDJ-3000 check, then make it default
### Q8 Cross-platform CI [waiting, 15]
### Later
- rc tests on the Mac: 7 macOS-only test failures (APFS case/NFD, tempo extremes, truncated playback, unicode paths).
- bench_analysis --dump pairs results off by one after a failed file.
- Stick read for Rekordbox-only sticks: plan docs/plans/stick-read-2026-11-22.md.
- Clean old worktrees and cargo caches.

## Run
status: idle · no queue run. Next only on Merlin's go.
review: https://dev.tail87069a.ts.net:4220 (lab cards)

## Rundown
2026-11-22/23: GPL licence, README, blog published; writer v2 + stick UI r1-r2 + play from stick + duplicates/reconnect/export merged to master and pushed (build 2148 installed). Analysis truth: key beats rekordbox, BPM tie, 5x faster. Phrase r2 and genre r1 not good enough (kept off).
Running: genre r2 (two runs), stick UI r3.
Next: Merlin reviews stick-ui-r3, makes the FF-GOLDEN export; Mo tests writer v2 on a CDJ.
