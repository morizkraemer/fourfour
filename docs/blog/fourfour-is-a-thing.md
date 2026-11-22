# Blog outline: "fourfour is a thing"

Outline only, not published. For merlins-internet.com (/blog), first post tagged `fourfour`.
Checked 2026-11-18: no fourfour post exists yet (20 entries; latest: prunr overhaul, pad + demos in TestFlight).

## Voice notes (from "this is bounce!", "pad - pureref for ipad", "prunr is in alpha!", "i reverse engineered Ableton Live...")
- All lowercase, casual, run-on sentences, honest asides ("i mean come on", "dafak?"), a few emoji, 🍃 sign-off.
- Shape: the problem first ("first why?"), then the existing tools and why each is a compromise, then "so we built our own", then screenshots with short captions, then "still in dev" + a call to test.
- Short, no marketing claims, no feature tables. Numbers only when they're fun.
- Ends with a link and a small ask ("help me test it pws?").

## Title options
1. fourfour is a thing (rec)
2. me and my brother are building a rekordbox alternative
3. i just want to put songs on a usb stick

## Outline

### 1. The hook (2-3 lines)
- "i did a thing. well, we did": fourfour is co-authored with Mo, his brother ([github.com/morizkraemer](https://github.com/morizkraemer)). both djs, both in software.
- One-line pitch: a lightweight, free, open source app for the boring half of rekordbox: library and usb sync. quick, modern, just works.

### 2. First why? (Merlin's angle, 2026-11-22)
- we couldn't believe how bad rekordbox is, and how many djs rely on it every day for quite simple things: library management and usb sync.
- most djs use rekordbox for exactly that, not for performance.
- the ui is stuck in the last decade, slow and clunky. going from "make a setlist" to "it's on the stick" takes way too long.
- the performance side is great and works well with pioneer's own controllers and cdjs. we're not trying to replace that (yet).
- so this is only the management and sync side. to actually replace rekordbox there, we have to crack two things: track analysis (bpm, grid, key) and the format the cdjs read off the usb.

### 3. Others doing the same thing
- several people are building something similar. we didn't find a nice open source one yet.
- closest match: ora dj. does all the same things (even in the free tier) and more, and is more polished. but it's closed source, so we keep going.
- CHECK: rbxport (GitHub, GPL-2.0+, Tauri, same "fewer features than rekordbox" pitch, own analysis, OneLibrary export, ~400 stars) is open source and very close; see docs/research/competitors-2026-11.md. Merlin decides whether and how to mention it.

### 3b. Two brothers, one repo, lots of agents
- also just a great project for the two of us to build something together.
- learning collaboration, ci, tests, code review in the age of ai agents doing most of the typing.
- rough split: Mo on the usb / cdj side (the database and files the players read, syncing, testing on real hardware), Merlin on the app (library, player, analysis, ui).
- optional tie-in to the Ableton post: lots of Claude, lots of iteration, "no thats wrong, it actually behaves like this".

### 4. Fun facts (fourfour vs rekordbox)
Same Mac (M5 Pro, 24 GB). rekordbox 7.2.14 timed by Merlin with a stopwatch on 2026-11-18 (two scripted launches agree: 28 s and 33 s to the main window). Test folder: his 2025 DJ folder, 512 tracks, 16 GB.

| | fourfour | rekordbox 7.2.14 |
|---|---|---|
| Start to a usable library | about 0.6 s (10k tracks) | 29 s (8 s until the splash screen, 24 s first paint) |
| Quit | ? | 14 s |
| Import the 512-track folder | not logged (app start to analysis start 27 s, incl. clicks) | 4 s |
| Analyse the 512 tracks | 25:59 min for 510 (about 3 s a track; log, mostly on battery) | 34:19 min (about 4 s a track) |
| App size | 84 MB | 2.4 GB |
| Library file at 50k tracks | 59 MB | ? |

Fun-fact lines that need no rekordbox number:
- "analysis runs fully local in rust with a small open beat model (Beat This!). no python, no cloud".
- "on a public test set the key is right about 79% of the time and the bpm about 92%".
- "the library file used to be 7.5 GB at 50k tracks. now it's 59 MB".
- "startup went from 188 s to 3.3 s to 0.6 s over a few weeks of nerding out" (30k first paint early on -> now).

Numbers I left out on purpose:
- 351 tracks a minute: the bench log itself flags it as doubtful (the same night measured 177). Use 177.
- Key accuracy vs rekordbox: different test sets, so not comparable.

### 5. What's in it right now (screenshots carry this)
- Library + big waveform player, hot cues (number keys 1-8 fire them, quantized to 1/16), beat grid with downbeats, inspector that pops open when you pick a track.
- USB export for the CDJs (phrase it per Mo's check below).
- Screenshot list: library view, player with grid + cues, inspector, settings, a stick in the sidebar. Workers can grab these headless from the review build; final picks are Merlin's.

### 6. Where it stands
- Early testing: alpha builds, Mac only, unsigned (alpha.2 = build 1974).
- Next: the USB sync flow ("plug in = sync"), a real name, signing, then a public beta.
- "fourfour" is a working title (say so).
- Free and open source forever.

### 7. Outro
- Ask: DJs with a Mac and a CDJ setup who want to test, ping us (link: to decide).
- 🍃

## Before it goes public (Mo should OK)
1. Credit line and link as above (co-author, github.com/morizkraemer). Photo or not.
2. What we claim about CDJ playback: which players and firmware it was actually tested on (CDJ-3000 round trip is still open in STATE). Rec: only claim what Mo confirms.
3. How we talk about the Pioneer format work ("reverse engineered" vs "works with"). Pioneer, rekordbox and CDJ are trademarks: name them only to describe compatibility, no logos.
4. "Open source forever" implies a licence: the repo needs one (check LICENSE) and Mo agrees on which.
5. Repo link and download: the repo is public, releases are private drafts. Rec: link the repo once the licence is in; testers sign up instead of a download link.

## Open
- fourfour import + analysis of the same 512-track folder (Merlin, 2026-11-18). Same power state as the rekordbox run.
- Which rekordbox analysis options were on (phrase, vocal, key) when it took 34 min: say so in the post.
