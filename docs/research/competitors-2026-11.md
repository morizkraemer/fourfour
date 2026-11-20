# Competitor sweep, 2026-11-20

Web research only, one pass. Fetched via README/homepage reads, partly model-summarised. "unk" = not verified. Star counts are from repo headers at fetch time. No commit dates were captured for most repos (unk). Nothing here was tested on hardware.

## 1. Table

USB = writes a CDJ-playable stick without Rekordbox. Own = own BPM/grid/key analysis.

| Name | Link | Open / licence | Price | Platforms | USB without Rekordbox | Own analysis | Activity / maturity | UI |
|---|---|---|---|---|---|---|---|---|
| rbxport | github.com/chrisle/rbxport | yes, GPL-2.0+ | free | Mac, Win, Linux | yes, OneLibrary USB + PRO DJ LINK; crates also write export.pdb, ANLZ, waveforms (via rbx-cli README). Hardware: CDJ-3000, XDJ-RX3 (author; USB vs LINK unk) | yes (grid, key); also opens Rekordbox 6/7 library | 407 stars, 1814 commits, launched 2026-10-07 | own GUI, "inspired by rekordbox" (Tauri 2, React/TS + Rust) |
| rbx-cli | github.com/lorenzobersano/rbx-cli | yes, GPL-2.0+ | free | Mac, Linux | yes, export.pdb, exportExt.pdb, exportLibrary.db, ANLZ DAT/EXT/2EX, artwork. Tested on CDJ-2000NXS2 (HFS+ stick) | yes, via rbxport crates (grid matched RB on 16/20, key 8/20 by its own test) | 2 stars, 6 commits, early | CLI |
| DJSync | github.com/Pikkol0/djsync | yes, GPL-3.0 | free | Win, Mac (Apple silicon), Linux | yes, exportLibrary.db + export.pdb + ANLZ waveforms. No published hardware test list | yes | 5 stars, 464 commits, beta (1.4.x rc) | own GUI (Python, PySide6), portable from the stick |
| dj-usb-tkit | github.com/haivala/dj-usb-tkit | yes, AGPL-3.0+ | free | unk | claims Pioneer + Engine USB; format path unk | yes (BPM, key, waveforms) | 58 stars, active | Tauri |
| baken | github.com/M-Igashi/baken | CLI MIT; Mac app paid | CLI free, app unk | CLI all OS, app macOS 14+ | export.pdb + OneLibrary from a Rekordbox collection.xml; needs Rekordbox once | mostly relies on Rekordbox | 71 stars, 348 commits, mature-ish | CLI + native Mac app |
| Sleeve | github.com/alii/sleeve | yes, GPL-2.0+ | free | unk | no, edits Rekordbox library | relies on Rekordbox | 21 stars | own GUI, built on rbxport crates |
| FableGear | github.com/fabledharbinger0993/FableGear | yes, MIT | free | macOS | claims export.pdb + OneLibrary + ANLZ; "not yet confirmed on a physical deck" | yes, Rekordbox fallback | 6 stars, 1157 commits | Flask + pywebview, MCP server |
| booth | github.com/jwilkins/booth | unk | unk | unk | "just works alternative to RekordBox, tested on CDJ-3000X/3000/1500X"; USB writing unk | unk | 28 stars, Rust | unk |
| recordbox | github.com/adamsardo/recordbox | unk | free | macOS | claims CDJ/XDJ USB export, unverified | unk | 57 stars, Rust/egui | egui |
| acrilique/pino, Radexito/DjManager, cue-drive, cdj-tool, fragmede/rekordbox-pdb | github (search) | unk | free | unk | all claim some Pioneer export; unverified, 1-3 stars | unk | hobby | various |
| cdj-media | github.com/johlrogge/cdj-media | yes, MIT | free | Rust crate | not implemented (1 commit) | none | 0 stars | library |
| openDJ | github.com/sebastian-beca/opendj | yes, GPL-2.0+ | free | Mac, Linux, Win | no USB export; full DJ app, imports Rekordbox | yes | 226 stars, 831 commits, beta | Rust + Slint |
| OpenCrate (releases) | github.com/WhatMeansGithub/OpenCrate-releases | GPL-3.0 stated; repo holds builds, source location unk | free | Mac (arm + Intel), Win x64, unsigned | claims both Rekordbox stick formats (OneLibrary + Device Library) and Engine DJ; file-level formats and player tests unk | yes (tempo, grid, key; key "matches Rekordbox ~6 in 10") | 0 stars, 40 commits, alpha, "formerly Krate" | own windowed app (tabs, sidebar) |
| openCrate (Calvin) | github.com/Calvin-Zikakis/openCrate | yes, GPL-3.0+ | free | Win 11, macOS (Apple silicon dmg) | no, M3U8/CSV/XML or writes Rekordbox master.db | no, Rekordbox owns analysis | 0 stars, 160 commits, v0.1.1 | Tauri 2 + React |
| Lexicon DJ | lexicondj.com (company Rekordcloud) | closed (unk) | Free / Essential / Ultimate subscription, lifetime option, amounts unk; USB sync free on all plans | Mac, Win, Linux, mobile app | yes, OneLibrary + Device Library on one stick | yes (BPM, grid, key, energy) on paid plans | established, active | own library manager |
| Ora DJ | ora-dj.com | closed (unk) | Free tier incl. USB export (4 drives at once); Ora+ 59 USD/yr | Mac, Win, iOS, Android | USB export on free tier; format and Rekordbox need unk | yes (key, BPM, grid, energy, vocal) | 1.0 released 2026-11-17 | full DJ app with decks + mixer |
| Mixxx | github.com/mixxxdj/mixxx | GPLv2 | free | Linux, Win, Mac | no. Reads Rekordbox USBs; OneLibrary issue #15556 open, no PR | yes | 2.5.6 on 2026-03-27 | own DJ app |
| Engine DJ | enginedj.com | closed | unk | desktop | Engine hardware only, not Pioneer | unk | comparison only | own |
| DJ.Studio | mixedinkey.com/dj-studio | closed | one-time 129-209 USD (Black Friday) | unk | none mentioned | claims AI beat detect | unk | timeline editor |
| Rekordbox | rekordbox.com | closed | free basics; 10-30 EUR/month tiers | Win, Mac, mobile | baseline | yes | 7.2.20 (Nov 2026) | baseline |
| Traktor Pro 4, djay Pro, VirtualDJ | AlphaTheta OneLibrary page | closed | n/a | n/a | named as OneLibrary partners; export details unk | yes | n/a | DJ apps |
| Library tools: pyrekordbox, rekordcrate, crate-digger | github.com/dylanljones/pyrekordbox, Holzhaus/rekordcrate, Deep-Symmetry/crate-digger | MIT / unk / EPL-2.0 | free | n/a | read-side only (pyrekordbox reads OneLibrary; no export.pdb) | no | pyrekordbox active, 519 stars | libraries |

Not found / unk: Rekord Buddy (Mixed In Key page 404), MIXO (not searched), Serato and Traktor (baseline only, not researched).

## 2. Closest to fourfour

1. **rbxport** (closest). Free, open, Tauri 2, "fewer features than Rekordbox" vision, own analysis, OneLibrary USB export, Mac/Win/Linux, 407 stars after weeks. Same pitch as ours. Differences (as far as verified): ours is macOS-first with a Rust analyser (Beat This! ONNX, key 78.9% exact on held-out) and a player strip with hot cues; hardware proof on CDJ-3000 is ours (via Mo). rbxport's hardware list is CDJ-3000 and XDJ-RX3, mostly PRO DJ LINK. Licence GPL-2.0+ vs our permissive-only analysis stack: no code can flow either way without a licence decision.
2. **DJSync**. Same job (PDB + OneLibrary + ANLZ, own analysis), Python/PySide, GPL-3.0, beta, no hardware test list. Differs: Python, heavier, portable-on-stick idea. We are faster and native.
3. **OpenCrate (releases)**. Rekordbox + Engine sync, own analysis, both stick formats, Mac/Win. Differs: positioned as library manager bridging Rekordbox and Engine; our scope is smaller (scan, analyse, play, sync). Closed/unclear source, alpha.
4. **Lexicon DJ**. The commercial incumbent for "manage library + sync to CDJ without Rekordbox". Subscription; analysis is paid. We differ on price (free forever), size, and speed, but they have years of format and user-edge-case knowledge.
5. **Ora DJ**. Free USB export, own analysis, 1.0 on 2026-11-17. Is a full DJ app with decks; we are a prep tool. Paid cloud and Ora+ upsell.

Also notable: dj-usb-tkit (Tauri, AGPL, 58 stars), baken (Rekordbox-dependent but strong format knowledge), booth and recordbox (unverified).

## 3. Learn / borrow

- Format knowledge: rbx-cli's README documents what worked on a CDJ-2000NXS2 (HFS+ stick quirks, empty artwork issue). baken cites issues for CDJ-3000, NXS2, NXS, XDJ-AZ with OneLibrary read only on XDJ-AZ (unverified by us). FableGear PR #222 states a CDJ-3000 on firmware 3.2x reads only the DeviceSQL export.pdb, not OneLibrary (unverified; compare with Mo's tests).
- Deep Symmetry analysis site (djl-analysis.deepsymmetry.org) is the community format reference; crate-digger EPL-2.0, Kaitai specs useful as a cross-check.
- Max Compatibility mode (rbxport): FLAC/M4A written as WAV for older CDJs. Candidate sync option.
- Lexicon: writes both formats on one stick; sync free on every plan; energy value. Ora: export to up to 4 drives at once.
- Safety habits: OpenCrate backs up the library before each change; DJSync warns "keep a copy of the drive". Matches our "never delete files we did not write".
- Test discipline: rbx-cli compares grid/key against Rekordbox on 20 tracks; same style as our r180 comparison.
- Licence caution: nearly all open competitors are GPL-2.0+/3.0/AGPL. Reading them for format facts is fine; copying code would force GPL on fourfour. MIT ones (cdj-media, FableGear, baken CLI, pyrekordbox) are borrowable in principle, but cdj-media has no code yet.
- Marketing facts: Mixxx has an open OneLibrary request (#15556) and no writer, so the open-source world is still waiting for this; AlphaTheta says "full compatibility between applications isn't guaranteed" and lists only DJ-app partners for OneLibrary (no licence terms found).

## 4. Naming

Taken in this space (avoid): **OpenCrate / openCrate / Krate** (two repos), **rbxport**, **rbx-cli**, **DJSync / djsync**, **baken**, **Sleeve**, **Lexicon**, **Rekordcloud**, **recordbox**, **booth**, **cue-drive**, **openDJ**, **Engine DJ**, **Ora DJ**, **DJ.Studio**, **Rekord Buddy**, **FableGear**, **cdj-media**, **cdj-tool**, **DjManager**, **pino**, **dj-usb-tkit**, **prospektdj**, **Mixxx**.

Crowded patterns: anything with "rekord" / "rbx" / "record" (rekordbox play-on-words), "crate" / "crates" (OpenCrate, rekordcrate, crate-digger, Krate), "open" + DJ, "sync", "cdj", "pioneer", "usb", "stick", "deck", "booth", "bake/baken", "kit/toolkit". "Pioneer" and "Rekordbox" are trademarks (our bundle id already flagged). Note "fourfour" itself: unchecked for collisions beyond this sweep; the baken issue #115 already refers to morizkraemer/fourfour by name.

## 5. Sources

- https://github.com/WhatMeansGithub/OpenCrate-releases
- https://github.com/Calvin-Zikakis/openCrate
- https://github.com/chrisle/rbxport
- https://edmlife.com/rbxport-is-a-free-open-source-rekordbox-alternative-for-usb-djs-what-it-does-better-and-what-it-wont-replace/
- https://github.com/lorenzobersano/rbx-cli
- https://github.com/Pikkol0/djsync
- https://github.com/haivala/dj-usb-tkit
- https://github.com/M-Igashi/baken and issue https://github.com/M-Igashi/baken/issues/115
- https://github.com/alii/sleeve
- https://github.com/fabledharbinger0993/FableGear (PR #222)
- https://github.com/sebastian-beca/opendj
- https://github.com/johlrogge/cdj-media
- https://github.com/kuwa72/rkbdb2xml
- https://github.com/jwilkins/booth , https://github.com/adamsardo/recordbox (search results only)
- https://www.lexicondj.com/ , https://www.lexicondj.com/onelibrary , https://www.lexicondj.com/pricing
- https://ora-dj.com/
- https://github.com/mixxxdj/mixxx (issue #15556)
- https://alphatheta.com/en/onelibrary/ , https://rekordbox.com/en/
- https://mixedinkey.com/dj-studio/ , https://www.enginedj.com/ , https://rekord.cloud/
- https://github.com/dylanljones/pyrekordbox , https://github.com/Holzhaus/rekordcrate , https://github.com/Deep-Symmetry/crate-digger
