# rbxport and Ora DJ deep dive, 2026-11-20

Web research plus a read-only shallow clone of chrisle/rbxport at v1.10.0 (2026-11-07). Nothing run, nothing tested on hardware. "UNVERIFIED" = not confirmed from a primary source. Page reads were done by a fetch agent (model-summarised) except rbxport files I read myself. No web search was available, so no Reddit/forum/press coverage was found for either product.

## 1. rbxport

### OSS status (verified)
- Open source, GPL-2.0-or-later, whole repo: https://github.com/chrisle/rbxport/blob/main/LICENSE (copyright Chris Le 2026). `LICENSING.md` repeats it. Workspace `Cargo.toml` says GPL-2.0-or-later; I checked all 18 `rbl-*` crates and `src-tauri`: every one inherits it (`rbl-deck` sets it explicitly).
- Vendored Rubber Band 4.0.0 (GPL-2.0+ or commercial) sits in `crates/rbl-deck/vendor/rubberband`; project takes the GPL. `--no-default-features` drops key shifting.
- Unclear: a comment in `rbl-deck` mentions an "MIT build"; the analysis page says "GPL v3". Treat all as GPL.
- No closed or paid parts found. Homepage says "Always free" (PayPal donate link). No CLA/DCO in CONTRIBUTING.md (inbound terms unstated).
- Telemetry (https://rbxport.com/privacy, 2026-09-25): site uses Google Analytics; app has optional anonymous usage stats (default UNVERIFIED); bug reports go to report.rbxport.com; Sentry crash reporting is a build option (`src-tauri/Cargo.toml`: "keep optional telemetry features off"). Update check UNVERIFIED.
- Traction: 407 stars, 76 forks, 204 open issues, 25 open PRs (fetched 2026-11-20). Contributors UNVERIFIED. `release-notes.json` has 24 entries, v1.1.0 on 2026-10-07 to v1.10.0 on 2026-11-07: about 10 minor releases in 31 days. GitHub Releases is empty; binaries come from rbxport.com. Downloads UNVERIFIED. Credits "special thanks" to people incl. AlphaTheta (nature of help UNVERIFIED).
- Positioning (README): "fewer features", Rekordbox-inspired library manager; scope = library, USB export, PRO DJ LINK. Yet `rbl-deck` gives it a deck (cue pads, loops, EQ kill, master tempo via Rubber Band), so it is more than a prep tool.

### Feature table vs fourfour
| Area | rbxport | fourfour |
|---|---|---|
| Library source | Opens existing Rekordbox 6/7 library in place (versions UNVERIFIED) | Scans folders, own DB, Rekordbox import (docs/architecture/rekordbox-import.md): similar |
| Analysis | Classical DSP: onset autocorrelation BPM with 132 prior, kick-aligned grid, beat 1 from arrangement, key via edmkey. Own 155-track Beatport test: BPM 155/155, grid 142/155, key 141/155 (self-reported, model not neural) | Better on method (Beat This! ONNX, key 78.9% exact held out). Their numbers are on a different set, not comparable |
| Speed claims | Starts in 1.5 s on 38,000+ tracks, under 200 MB; CI perf budgets (`perf-budgets.json`, gzip bundle caps) | Have: 50k startup 1.35 s. Missing: enforced perf budget in CI |
| Waveforms | 3 bands (<200 Hz, 200 Hz-2 kHz, >2 kHz), 150 cols/s, PWV3-7 incl. 2EX; overview calibrated against Rekordbox, mean error 0.6-2.2 px on 9 tracks | Have the same 3-band 150 col/s design (not yet tried on a player). Equal |
| Cues / grid editing | Hot, memory, loops, cue names/comments/colours, beatgrid edit | Have hot cues; loops and memory cues need checking (UNVERIFIED) |
| Playlists | Folders, playlists, intelligent playlists (exported by rule), history, ratings, My Tags, undo/redo | Missing: smart playlists, undo (library-wide) |
| USB export | export.pdb + exportLibrary.db + exportExt.pdb (My Tags) + ANLZ DAT/EXT/2EX + artwork + DEVSETTING/MYSETTING | Have PDB + OneLibrary + ANLZ + artwork. Missing: exportExt.pdb, settings files, 2EX (Mo's open item) |
| Sync model | Sync Manager: tick playlists per stick, SYNC writes only, Import (cues, grids, history, ratings) is a separate button; space preview per stick; "Verify all files"; format FAT32/HFS+ from the UI | Better on design intent (plug in = sync, per-playlist Keep updated). Missing: import back from stick, format stick, verify mode |
| Safety | Staged write, verify, then commit; interrupted export recovered next sync; previous library kept until the new one checks; manifest at `PIONEER/rbxport/manifest.json`; never deletes files it did not export | Same rule. Missing: staged publish + recovery journal (Mo's lane) |
| Max Compatibility | Off by default. Converts FLAC/ALAC (and incompatible mono/stereo) to WAV (16-bit 44.1k, default), AIFF or MP3 320 CBR; compatible MP3/PCM copied as-is; surround refused; 6 GiB cache cap | Missing (a FLAC check exists in docs/architecture/cdj-checklist-2026-10.md) |
| Multi-stick | Several sticks, shared conversions, Auto Eject | Backend built (multi_sync.rs), no UI |
| PRO DJ LINK | Yes (LINK EXPORT, NFS, dbserver crates) | Out of scope |
| Backups | Library backup crate, protection lock | Not compared (UNVERIFIED) |
| Platforms / stack | Mac, Win, Linux; Tauri 2, React/TS, 18 Rust crates, SQLCipher; Symphonia, Rubato, cpal, LAME. App size UNVERIFIED | Mac first; Svelte 5. Missing: Win/Linux builds (CI pending) |

Hardware (https://rbxport.com/ and docs/reference/link-testing.md): LINK export on XDJ-RX3 physical (fw 1.20); CDJ-3000 fw 3.20 only in an emulator booting vendor firmware; XDJ-AZ one inconclusive physical rating check. Homepage: "testing every player with every firmware requires hardware we don't have." So their hardware proof for USB playback is thinner than ours (CDJ-3000, via Mo).

### Standout UX ideas
- Sync Manager as a tick-list with a live space bar per stick, and a pre-sync review of tracks with missing audio (cancel or skip).
- Staged "Already up to date" no-op, per-stage progress (checking, converting, copying, database, verifying, saving), "Stop all exports".
- Device tree: a stick expands into Device Library / OneLibrary with its playlists; right-click Import Playlist (copies missing tracks into a folder, never overwrites).
- Hard stops that protect data: a cue added on a player blocks SYNC until imported; a rating changed on a player stops SYNC until imported.
- Drag onto a stick row: tracks export, playlists/folders export.
- Missing File Manager (find, relocate, remove). Bug report button in the status bar with full context.
- Product rule written down: "fewer features" plus a performance budget gating CI.

## 2. Ora DJ

### OSS status
- Closed source. EULA section 7.1 bans reverse engineering: https://www.ora-dj.com/terms-and-conditions (2026-10-13). No repo for the app; GitHub is only used for update downloads (https://www.ora-dj.com/privacy).
- Provider Heyora UG, Berlin (founder Yannick Feige, /about). EULA calls it a free beta software at section 1.2; 1.0 shipped 2026-11-17; first public release v0.14.0 on 2026-05-27.
- Terms to note: remote deactivation and periodic check-in (13.2, 13.6), no data mining or AI training on the software (7.2), user keeps content (8.7), analysis local (8.5). PostHog and Sentry opt-in. ACRCloud lookups only when the user starts Metadata Repair.

### Pricing and features (https://www.ora-dj.com/pricing)
- Free: unlimited library, set planning, Groove Match, 2 and 4 decks, controllers/MIDI, USB export to up to 4 drives at once, PRO DJ LINK, mobile apps (25 tracks).
- Ora+ 59 EUR/USD per year: recordings, unlimited mobile sync, Stem Studio, Metadata Repair, Duplicate Cleanup. Cloud Library 9.95 per month (10,000 streamed tracks). Whether Cloud includes Ora+ is contradictory across pages (UNVERIFIED).
- Platforms: macOS 12+ universal (374 MB, 8 GB RAM min), Windows 11 (260 MB, "public beta"), iOS 16.4+ (57 MB), Android.

### USB export (https://www.ora-dj.com/docs/library-management/usb-export)
- Docs name FAT32 MBR and a 4 GB file limit. They never name export.pdb, OneLibrary or Engine: file format UNVERIFIED. Rekordbox is read only for import from `master.db`; whether export needs Rekordbox UNVERIFIED (no page says so). Hardware tested: not stated.
- Options: "Replace and sync" (player libraries, cues, grids, settings), key/BPM/energy appended to titles on the drive only, "Verify and eject", "Recover sync", reuse of unchanged audio, 4-drive parallel progress. Apple Lossless cannot be exported (v0.47).

### Feature table vs fourfour
| Area | Ora DJ | fourfour |
|---|---|---|
| Analysis | Key, BPM, grid, energy 1-10, vocal percentage (filterable), local; Groove Match embeddings (CLAP derivative, ONNX, MS-PL); Kim Mel-Band RoFormer vocal model (Core ML) | Have key/BPM/grid; energy landed off by default; missing: vocal share, similarity |
| Library | Metadata Repair and Duplicate Cleanup show proposals, change nothing until accepted | Missing both (STATE: 144 of 510 tracks are likely duplicates) |
| Rekordbox import | Pick scope tree, merge mode; match by exact path, filename, artist/title, duration within 2 s; ambiguous match stops and lists tracks | Have import; matching ladder worth comparing |
| USB | 4 drives parallel, recover sync, title tags | Backend only |
| Players | Full DJ app: 2/4 decks, stems, recording, phone app | Out of scope |

### Standout UX ideas
- Energy/key/BPM written into the title on the stick only: lets CDJs without those columns sort by them.
- Proposal lists (accept to apply) for any bulk edit.
- Visual set canvas with chapters (warm-up, build, peak, out); recordings with a transition list ("click a track to jump to 10 s before it comes in").
- Free tier carries USB export, which is the main hook. Sets the price expectation for us (free).

## 3. What we can copy

### (a) Build ourselves (ideas, always fine)
1. Max Compatibility toggle: convert FLAC/ALAC to WAV/AIFF/MP3 per target, cached, surround refused, "never convert what already plays".
2. Pre-sync checklist: missing-audio review, space preview per stick, Verify mode, "Already up to date".
3. Staged write plus verify plus recovery on next sync; keep previous generation until new passes.
4. Import back from a stick: cues/grids/history/ratings, and hard stop on player-edited cues (fits our open "reverse direction" question in docs/ui/usb-sync-decisions.md).
5. Smart playlists (rule exported as its tracks at sync time) and library-wide undo.
6. Proposal-style Duplicate Cleanup (we have the dup data), Missing File Manager, in-app bug report with context.
7. CI perf budget (startup, first rows, bundle size), like `perf-budgets.json`.
8. Title tagging with key/BPM/energy on the stick (optional).

### (b) Code reuse
- None. rbxport is GPL-2.0-or-later throughout (all 18 crates + app + vendored Rubber Band); Ora is closed. Reading is fine; copying any file would pull fourfour into GPL. Do not paste from `rbl-pdb`, `rbl-onelibrary`, `rbl-anlz`, `rbl-export`.
- Permissive pieces they depend on, usable directly from upstream, not from rbxport: Symphonia (MPL-2.0, check), Rubato (MIT), cpal (Apache-2.0), LAME (LGPL, so only via dynamic link: verify before use) - licences from memory, UNVERIFIED, check each before adding. Rubber Band needs a commercial licence for us (we already use Signalsmith Stretch, MIT).
- The only transferable asset is facts, below. Their docs are the best format write-up I saw: `docs/reference/usb-export-db.md` (1768 lines). Treat as a checklist for Mo, cite facts, do not copy prose.

### (c) Format facts to check against Mo's writer (all from rbxport docs, mostly unverified by us)
- CDJ-3000: their tested path is export.pdb + ANLZ; they say OneLibrary is not shown to be read (fw 3.20, emulator). CDJ-3000X fw 1.40 "thought to browse only exportLibrary.db" (static analysis, hypothesis). Matches the FableGear claim in competitors-2026-11.md. Compare with Mo's CDJ-3000 results and decide whether we always write both (we do).
- Ratings: write 0-5 in both DBs; x51 (XML scale) shows no stars on CDJ-3000X (#347).
- Every export.pdb table needs an index page and a named empty-candidate page; all 20 tables must exist or Rekordbox hides the volume ("Device library is corrupted").
- Pages over 255 rows: dirty-bit words at 0x20/0x22 must match Rekordbox, or Rekordbox 6.8.6 crashes (#349).
- Track/artist/album/property rows carry slot*32 at byte 2: CDJ-3000 ignores it, NXS-era players do not.
- Export ids must be stable across syncs (decks cache by id).
- OneLibrary `menuItem`/`category`/`sort` rows decide browse columns; names need U+FFFA/U+FFFB markers or players show English.
- Unused string slots point at an empty string, slot 7 = "ON" for hot-cue auto-load; album id at 0x0c; playlist tree rows are 5 words, sort_order per folder.
- Players write `PIONEER/rekordbox/RBFLTR.DAT` to the stick: never diff a stick that has been in a player.
- exportExt.pdb (My Tags) is not in our writer; `playlists3.sync` and DEVSETTING/MYSETTING are Rekordbox-side.
- Older players: XDJ-1000 builds browse from tables 17/18 (#619); CDJ-900NXS lockup (#182); CDJ-2000NXS freeze E-8709 (#321). Issues cited by their docs, not checked in the tracker.
- ANLZ path hash from final audio path, PPTH rewritten in every companion: same as our known gotcha.

## 4. Top 5 recommendations

1. **Max Compatibility mode (FLAC/ALAC to WAV/AIFF/MP3 on sync).** Effort: M (about 1 week; converter, cache, size estimate). Touches Mo's lane: yes (sync engine, file paths in DBs). Biggest practical gap: many libraries are FLAC.
2. **Safe sync: staged write, verify, recover, pre-sync checks (space, missing audio, Verify).** Effort: L. Mo's lane: yes. Matches our "plug in = sync" promise and protects sticks; rbxport has this done and documented.
3. **Audit the writer against rbxport's gotcha list** (stable ids, 255-row dirty bits, slot*32, ratings 0-5, index/empty-candidate pages, menu markers) and decide on exportExt.pdb. Effort: S-M (1-2 days reading, as a checklist for Mo). Mo's lane: yes, read-only for us.
4. **Smart playlists plus library-wide undo** (rules evaluated at sync time). Effort: M-L. Mo's lane: no (small sync hook: export the evaluated tracks). Gap vs both competitors in "organize" scope.
5. **Proposal-style Duplicate Cleanup and Missing File Manager.** Effort: M. Mo's lane: no. We already know 28% duplicates in a real folder; Ora does it under paid Ora+, we can ship it free.

Also noted, not ranked: CI perf budgets (S, no), import back from stick (L, yes, tied to open design question), title tagging with key/BPM/energy (S, yes).

## 5. Sources
- https://rbxport.com/ , /analysis/ , /privacy
- https://github.com/chrisle/rbxport (LICENSE, LICENSING.md, CONTRIBUTING.md, README.md, release-notes.json, docs/user/usb-export.md, docs/reference/usb-export-db.md, docs/reference/waveform-analysis.md, docs/reference/link-testing.md, docs/development/product-decisions.md, perf-budgets.json, crates/*/Cargo.toml)
- https://www.ora-dj.com/ , /pricing , /changelog , /terms-and-conditions , /privacy , /download , /docs/library-management/usb-export , /docs/library-management/rekordbox-import-export , /legal/third-party-notices
- https://apps.apple.com/app/id6810398883
