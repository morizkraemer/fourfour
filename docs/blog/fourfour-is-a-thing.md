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
- "i did a thing. well, we did": fourfour is co-authored with Mo, his brother ([github.com/morizkraemer](https://github.com/morizkraemer)).
- One-line pitch: a lightweight alternative to rekordbox. faster, a more modern ui, and free and open source forever.

### 2. First why? (the problem)
- Getting tracks onto a USB for the CDJs means rekordbox: heavy, slow, a whole pro suite (plus subscription upsell) when all you need is a library, analysis and syncing to your drives.
- There have been a couple of other projects like this. Most of them strip down rekordbox's existing ui, or rebuild all of it, just faster.
- Ours goes the other way: take the minimal feature set you actually need (library, analysing, syncing with your drives) and do those really well. Everything else stays out.

### 3. So we built it (the brothers part)
- Mo and Merlin co-wrote it. Rough split: Mo on the USB / CDJ side (the database and files the players read, syncing, testing on real hardware), Merlin on the app (library, player, analysis, ui).
- Fun angle: two brothers, one repo, Merlin travelling while Mo has the hardware.
- Optional tie-in to the Ableton post: lots of Claude, lots of iteration, "no thats wrong, it actually behaves like this".

### 4. Fun facts (fourfour vs rekordbox)
fourfour numbers are measured (Mac M5 Pro, 24 GB). The rekordbox column is empty: nothing in our docs measures it. Merlin times rekordbox once on the same Mac and library (see the ask below), then keep only the rows where the gap is fun.

| | fourfour (measured) | rekordbox (to time) |
|---|---|---|
| Cold start, 10k tracks, to first row | about 0.6 s (0.66 s until usable) | ? |
| Start with 50k tracks | about 1.35 s | ? |
| Import a 10k-track folder | about 0.5 s (re-import 0.1 s) | ? |
| Analysis (BPM, beat grid, key) | about 177 tracks a minute, first result in under 5 s | ? |
| Library file at 50k tracks | 59 MB | ? (its master.db + analysis files) |
| App download | about 97 MB DMG | ? (installer size) |

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

## Ask for Merlin (rekordbox baseline)
On the Mac, with a stopwatch, against a copy of the same 10k folder (don't touch the real rekordbox library, use a fresh test one if possible):
1. Cold start to a usable library.
2. Import the folder.
3. Analysis time for 100 tracks.
Plus the rekordbox installer size and its library folder size. Then the table fills in.
