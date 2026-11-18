# Blog outline: "fourfour is a thing"

Outline only, not published. For merlins-internet.com (/blog), first post tagged `fourfour`.
Checked 2026-11-18: no fourfour post exists yet (20 entries; latest: prunr overhaul, pad + demos in TestFlight).

## Voice notes (from "this is bounce!", "pad - pureref for ipad", "prunr is in alpha!", "i reverse engineered Ableton Live...")
- All lowercase, casual, run-on sentences, honest asides ("i mean come on", "dafak?"), a few emoji, 🍃 sign-off.
- Shape: the problem first ("first why?"), then the existing tools and why each is a compromise, then "so i built my own", then screenshots with short captions, then "still in dev" + a call to test.
- Short, no marketing claims, no feature tables. Numbers only when they're fun.
- Ends with a link (software page or a signup) and a small ask ("help me test it pws?").

## Title options
1. fourfour is a thing (rec)
2. me and my brother are building a rekordbox alternative
3. i just want to put songs on a usb stick

## Outline

### 1. The hook (2-3 lines)
- "i did a thing. well, we did": first project with Mo (his brother).
- One-line pitch: a small, fast music library for DJs that analyzes your tracks and puts playlists on a USB stick that plays on Pioneer CDJs. No Rekordbox.

### 2. First why? (the problem)
- Prepping a USB for a gig means Rekordbox: heavy, slow, cloud/subscription upsell, a pro suite when he only wants sync, play, organize.
- The actual workflow he wants: drop in a folder, it analyzes BPM / key / grid, pick playlists, plug in the stick, done.
- Short honest take on the alternatives (to fill, one line each): Rekordbox itself, Lexicon / Rekord Buddy style converters (still need Rekordbox or a paid tier), Engine DJ (other hardware). "its all a compromise" beat, like the pad post.

### 3. So we built it (the brothers part)
- Split: Mo does the hard format part (the USB database and files a CDJ reads, the sync engine, testing on real players). Merlin does the app: library, player, analysis, UI.
- Fun angle: two brothers, one repo, Merlin travelling while Mo has the hardware.
- Optional tie-in to the Ableton post: lots of Claude, lots of iteration, "no thats wrong, it actually behaves like this".

### 4. What's in it right now (screenshots carry this)
- Library + big waveform player, hot cues (number keys 1-8 fire them, quantized to 1/16), beat grid with downbeats.
- Analysis runs locally in Rust with a small open model (Beat This!) for beats, no Python, no cloud. Fun numbers: about 350 tracks a minute on his Mac; on a public test set the key is right about 79% of the time and BPM about 92%.
- Fast with big libraries (50k tracks opens in about 1.3 s).
- USB export for CDJs (see the Mo check below on how to phrase what's tested on hardware).
- Screenshot list: library view, player with grid + cues, inspector, settings, a stick in the sidebar. Workers can grab these headless from the review build; final picks are Merlin's.

### 5. Where it stands
- Alpha builds, Mac only, unsigned, shared privately for now (alpha.2 = build 1974).
- Next: the USB sync flow ("plug in = sync"), a real name, signing, then a public beta.
- "fourfour" is a working title (say so, it's honest and on brand).

### 6. Outro
- Ask: DJs with a Mac and a CDJ setup who want to test, ping him (link: software page or a signup page, to decide).
- 🍃

## Before it goes public (Mo should OK)
1. Mo's name, role and photo or not: how he wants to be credited (GitHub morizkraemer).
2. What we claim about CDJ playback: which players and firmware it was actually tested on (CDJ-3000 round trip is still open in STATE). Rec: say "plays on the CDJs we tested" only for what Mo confirms.
3. How we talk about the Pioneer format work (reverse engineered vs "compatible with"). Pioneer, Rekordbox and CDJ are trademarks: name them only to describe compatibility, no logos.
4. Linking the GitHub repo: it is public under Mo's account. Rec: no link until Mo decides.
5. Download: releases are drafts (private). Rec: no download link in the post; testers sign up instead.

## Also for Merlin
- The name: post says "working title". Fine to publish under it?
- Where testers go: software page entry (like pad) or a small signup page (like prunr).
