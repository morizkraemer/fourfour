# Ora DJ docs read-through, 2026-11-20

Read all 43 pages in the sitemap (https://www.ora-dj.com/sitemap.xml): library, set planning, getting started, PRO DJ LINK, controllers; performing only skimmed. Pages were read by fetch agents (model-summarised, quotes may be trimmed). Closed source, no hardware tested. "UNVERIFIED" = not stated in the docs or not retrievable. Builds on docs/research/rbxport-ora-2026-11.md; not repeated here: pricing, EULA, licence, rbxport.
Not retrievable: the Rekordbox export sections (Export to Rekordbox, Keep it in sync automatically, What gets exported) have headings but no body text on https://www.ora-dj.com/docs/library-management/rekordbox-import-export (fetched 3 times). No pages on iTunes XML or Engine import exist.
Performing, one line: 2/4 decks, mixer, FX, pads, stems (Ora+), external mixing, MIDI clock, recordings with a transition track list, 40+ controllers. Out of scope for us.

## 1. USB export
Ora (all https://www.ora-dj.com/docs/library-management/usb-export unless noted):
- Flow: connect drive, open USB Export in sidebar, select drive, tick playlists, read compatibility report (make compatible copies if needed), Sync, "review changes" list if asked, then "Verify and eject" (plain Eject if not offered).
- Filesystem advice: FAT32 MBR works on more players than exFAT; 4 GB per file on FAT32. No formatting from the app documented. Players, firmware, capacity and track limits: not documented.
- "Replace and sync" can replace player libraries, cues, grids and settings. For a drive prepared by another app: back up first, or use "Import it first" to pull its tracks and playlists into Ora.
- Repeat sync "reuses unchanged audio". Up to 4 drives for the same playlists, each with its own progress and result.
- Title format: key, BPM, energy can be added to the title via track display settings in USB Sync, with a preview. Changes titles on the drive only, library untouched.
- Rekordbox-prepared drives: open playlists and load onto a deck to play from the stick (read only). Import view: "Review available play history and supported track edits before you import them." Which edits and what history detail: UNVERIFIED.
- Failure: "Sync incomplete" shows "Recover sync"; keep the drive plugged, wait, then sync again. A failed track shows file and reason in result details.
- Not documented: what happens to tracks deleted or changed since last sync, what "Verify" checks, what the review list contains, export format (PDB or OneLibrary), exFAT limits.
- Compatible copies, https://www.ora-dj.com/docs/library-management/compatible-copies: right-click playlist or selection, "Prepare compatible copies". Presets: Club-safe AIFF (FLAC and Apple Lossless become AIFF; MP3, AAC, standard WAV/AIFF copied as is) and MP3 320 (existing MP3 kept). Dialog shows the plan first: counts to convert, keep, "need attention"; anything above 48 kHz is flagged, not silently downsampled. Destination: remembered Export folder or "Next to originals". Copies are real tracks with cues, loops, grids, tags and analysis carried over, no re-analysis. A playlist input yields a new playlist "named after the original with the format on the end" in the same folder, in arranged order, "the playlist to put on your USB". Summary gives a reason per skipped track; missing files are reported; "0 originals modified". Bit depth, sample rate and encoder settings: UNVERIFIED.
- Recommended order on the CDJ page, https://www.ora-dj.com/docs/cdj-connection: files a player cannot read from USB go through Compatible copies.
- MP4 rules, https://www.ora-dj.com/docs/library-management/importing-tracks: for USB, one mono/stereo AAC-LC stream at 44.1 or 48 kHz, at least 1 s; file copied as is including video; ALAC-in-MP4 excluded.
- Playlist export (lists only, no audio), https://www.ora-dj.com/docs/library-management/export-playlists: CSV (position, title, artist), plain text, Rekordbox XML, Traktor NML, M3U8. Default name playlist + date, remembers folder, deleted tracks dropped and positions close up, smart playlists exported as fixed current matches. "Copy as text" for a tracklist. Traktor NML whole-library export keeps folders (https://www.ora-dj.com/docs/library-management/traktor-export): title, artist, album, genre, BPM, grid, key, hot cues (8), loops, memory points, rating, play count, comments; no auto sync; single playlist lands at top level.

fourfour today: PDB + OneLibrary + ANLZ + artwork writer (Mo), multi-stick backend without UI (docs/architecture/multi-usb-sync.md), sync UI round 3 mockups, plug in = sync model (docs/ui/usb-sync-decisions.md). No file conversion, no title tagging, no recover flow, no verify.

Ideas:
- Compatibility report before sync (per track: plays / converts / blocked: FLAC, ALAC, over 48 kHz, over 4 GB, surround) with named presets. [Mo lane] (sync engine, file paths)
- Conversion creates a derived copy and keeps cues and grid by reference, never touches originals; mirrors the rbxport Max Compatibility idea but as an explicit playlist-level step. [Mo lane]
- "Verify and eject" as one button, plus "Recover sync" as a visible state after an interrupted write. [Mo lane]
- Per-stick optional title suffix with key, BPM, energy; library titles never change. [Mo lane]
- "Import it first" prompt when the stick was written by Rekordbox, before replace. Fits the open reverse-direction question. [Mo lane]
- Playlist export to M3U8 / CSV / Traktor NML is cheap and needs no sync work (S, not Mo).

## 2. PRO DJ LINK (https://www.ora-dj.com/docs/cdj-connection)
- What: players browse and load the laptop library; audio plays from the player's own outputs. Same pick-the-computer-as-source step as rekordbox LINK Export. Menus on the player: Playlist, Artist, Album, Track, Key, History, Search, and Matching (Groove Match ranked, "within 16% of the loaded track's BPM and in a related key").
- Requirements: Ethernet only, no Wi-Fi. macOS needs Ora v0.50+, Windows v0.52+. USB link only on models that support it (e.g. XDJ-RX3); Windows USB needs Pioneer's LINK Export driver. Two or more players need a switch; up to 6 players. Models and firmware: not listed (named: XDJ-RX3, XDJ-XZ "work too"). IP, ports, firewall, direct cable with one player: not documented.
- Files: WAV, AIFF, FLAC, MP3, M4A up to 60 minutes; FLAC works because Ora "prepares each track when a player loads it". A track must be analysed first.
- Setup: Settings > CDJ connection, pick Player connection, Connect; players show Connected, top bar shows "CDJ Link" with a count. Optional auto-detect (never shares on its own). App keeps the computer awake, must stay open, music drives attached. Disconnecting removes the library from players mid-set.
- Write back: "supported cue, loop and play history changes" are saved; beatgrid edits stay on the player.
- Errors: CDJ-DISCOVERY-01/02/04 (unknown messages, unreadable messages, duplicate player number); detection blocked when another DJ app holds the connection.
- Could a deck-less library app offer it? In principle yes: the players are the decks, the host only serves metadata and files, and Ora's page lists no deck dependency. Cost: own implementation of the protocol (database server plus file serving, not documented by Ora; rbxport is GPL so no reuse), real hardware per model and firmware, always-on app during a set. No hardware on our side except Mo's CDJ-3000. My judgement, UNVERIFIED. Recommend: not v1; revisit after USB sync is solid. [Mo lane]
- Ideas: none now beyond the write-back model (cues and history come back, grid does not), which is a good rule for stick import too. [Mo lane]

## 3. Imports (https://www.ora-dj.com/docs/library-management/rekordbox-import-export)
Ora:
- Entry: Settings > Library > Import Library, or first-run setup. Sources: Rekordbox (reads local `master.db` from a copy, never writes; default `~/Library/Pioneer/rekordbox` or `%APPDATA%\Pioneer\rekordbox`, "Change" for another folder), Rekordbox XML, Serato (DJ 4 library), Traktor (`.nml`), Apple Music (macOS), folders. No Engine, no iTunes XML, no Rekordbox USB import (USB is read through the USB Export view).
- Step 1 tree: Music Library (all tracks), Playlists (folders and playlists, a selected playlist brings its tracks even if Music Library is off), Tags (My Tag groups), smart playlists marked "smart".
- Step 2 merge: "Import new tracks", "Update tracks" (choose what to overwrite: BPM and cues, key, rating), "Import new playlists", "Update playlists based on name and folder location", plus a dialog "How playlists are imported".
- Carried over from Rekordbox: BPM, key, hot cues (8), memory points (10), rating, play count, comments, Rekordbox track ID (kept for export), My Tag structure. Beatgrid NOT imported; Ora builds its own and cues "settle onto the Ora DJ grid once a track is analyzed". Colours, loops, history: not mentioned.
- Serato: "play data, colors, cues, loops, and beatgrids"; smart crates stay dynamic if rules are supported, else fixed lists. Traktor: what carries over is not stated; Smartlists dynamic, fixed or skipped by rule support.
- Smart playlists: supported ones become dynamic playlists run against the whole Ora library (results can differ); unsupported rules skipped and reported; "snapshot" keeps current matches. Update matrix by name and folder: Static to Static replace tracks and order; Smart to Static convert to dynamic; Smart to Dynamic replace rules; Static to Dynamic skip and keep Ora's; Snapshot to either replace with static list.
- Matching ladder: exact file path, file name, artist and title, duration within 2 s. Ambiguous match stops the import and lists the tracks. Missing file (unplugged drive) comes in as "missing". Unsupported formats left out and counted. Same filename in two folders can make XML import ambiguous, so import the relevant folder first (https://www.ora-dj.com/docs/library-management/importing-tracks). Both key notations read (Camelot, musical).
- Re-import: only through the Update options above. Two-way sync: UNVERIFIED (export body missing); the docs state "automatic option is Rekordbox-only" on https://www.ora-dj.com/docs/library-management/traktor-export, so a Rekordbox "keep in sync" XML export exists.
- Apple Music (https://www.ora-dj.com/docs/library-management/apple-music): macOS only, "Connect Apple Music", sidebar source, playlists read only, streaming-only tracks listed but cannot play, known tracks reuse their cues and grids.
- Folder import (https://www.ora-dj.com/docs/library-management/importing-tracks): watched while the app is open ("appears within a few seconds"), rescan on launch, notification with Show (temporary filtered view) and "Playlist +". Recursive; artist and title guessed from "Artist - Title" when tags are missing; BPM, key, waveform, energy never read from files. Optional Inbox review (Settings > Music Files), rename and move rules for new files only, with preview. Dropping a folder lists already-tracked ones as "skipped".

fourfour today (docs/architecture/rekordbox-import.md): master.db, OneLibrary USB (no PDB-only sticks) and XML; carries tracks, tags, playlists with folders flattened, BPM, key, grids, hot and memory cues, loops, rating, colour. Not imported: smart playlists, My Tags, history. Path remap rules with preview, merge policy (user values kept, only empty fields filled, second import changes nothing), imported grid and cues locked against re-analysis. No Serato, Traktor, Apple Music import (UNVERIFIED beyond the doc). Folder drop exists.

Ideas:
- Show which fields an update would overwrite and let the user tick them (BPM and cues / key / rating), like Ora's Update tracks; ours today never overwrites.
- Stop and list on ambiguous matches (same file name in two folders) instead of picking one; check our matching ladder against Ora's duration step (2 s).
- Imported smart playlists as dynamic playlists with a "snapshot" fallback once smart playlists exist. (M)
- My Tags import with group structure. (S-M)
- Serato import is the biggest unserved path in the docs: cues, loops, colours, grids, crates. (L, not Mo)
- Inbox review for new files before they enter the library. (S-M)

## 4. Library health
- Duplicates (https://www.ora-dj.com/docs/library-management/duplicates): three match types: "Exact copies" (identical file even renamed or moved), "Same recording" (artist, title, version, similar length, across formats e.g. MP3 and WAV), "Related versions" (edits, remasters, live; excluded from bulk cleanup). Compare view: files, lengths, play counts, playlist usage, audition, disconnected drive vs file not found. Recommended keeper per set. Cleanup choices: "Keep files where they are" (archive in library) or "Move files to a backup folder". Keeper absorbs play counts and tags and replaces extras in playlists. Exact-copy cleanup re-verifies identity first. "Restore copies" from the Archive tab does not undo the merge or playlist changes. No option deletes files. Whether cues, ratings, colours carry to the keeper: UNVERIFIED. Free vs Ora+: the pricing page says Duplicate Cleanup is Ora+ (see rbxport-ora-2026-11.md); the docs page does not say.
- Missing and moved files (https://www.ora-dj.com/docs/library-management/moving-files): tracks identified by a fingerprint (hash of size plus a byte sample), not the path. Live reconnect within seconds while open, rescan on launch, new drive by adding the folder. Guards: reconnect only if the old path is gone (a copy imports as a separate track); a file matching two missing tracks stays missing. Weak spot: tags rewritten and file moved before a scan imports as new and loses cues. Folder "Relocate..." re-points and rescans by fingerprint (imported folders only). Red icon "File missing", sidebar "Library health" count.
- Metadata editing (https://www.ora-dj.com/docs/library-management/editing-metadata): multi-select editor with "mixed" state, only changed fields applied; inline edit. Metadata Repair: lookup, compare current vs suggestion, tick cover art, apply in bulk (source not named; the rbxport note says ACRCloud, started by the user). Edits go to the library. "Write metadata edits to audio files" is off by default; a separate file-tag refresh setting is also off. Zero ratings never overwrite library ratings. No undo documented.
- Backups (https://www.ora-dj.com/docs/library-management/backups): playlists, cues, tags, ratings, analysis, no audio. Daily (excl. waveforms, default on, keeps 7) and Full (incl. waveforms, default off, 1 kept); checks on launch and every few hours; waits during analysis or playback; snapshot before a format-changing update. Restore shows what is replaced, sets the current library aside first, can target another folder; newer-version backups are greyed out. Recovery screen if the library will not open, with the same list and a fresh-library option.
- Library location (https://www.ora-dj.com/docs/library-management/library-storage): library data separate from music, movable; "Use this library" never merges; exFAT can lose recent changes if unplugged while saving.

fourfour today: duplicate dialog on import approved (same fingerprint OR artist + title + duration within 2 s, "Skip duplicates" default; docs/ui/file-management-2026-10-29.md); relocate (search one folder, relink all) and a launch health check with a top-bar status decided there; Backup settings page decided there. Whether each is built: UNVERIFIED. STATE shows 144 of 510 tracks are likely duplicates, so a post-import cleanup has data to work on.

Ideas:
- Duplicate view for the existing library (not only on import) with the three Ora types, a recommended keeper, and archive instead of delete. (M)
- Fingerprint by size plus byte sample so moved or renamed files reconnect with cues intact; check what ours uses. (S-M)
- Never reconnect when two missing tracks match; keep the "copy vs move" guard. (S)
- Backup retention rules like Ora: daily, keep 7, pre-update snapshot, restore sets the old library aside. (S, matches our decided Backup page)
- Tag writing stays opt-in and off by default.

## 5. Other prep features (brief)
- Smart Filter vs reference track (https://www.ora-dj.com/docs/library-management/filtering): BPM range, key or compatible keys, tags, genre, vocal percentage; stack; "Save as dynamic playlist" carries key, genre, BPM, energy, rating, year, date added, play count, tags. Per-list filter state is not kept across lists.
- Dynamic playlists (https://www.ora-dj.com/docs/set-planning/playlists-and-folders): AND/OR groups, nesting; folders show nested tracks combined; Quick access 12 slots; M3U/M3U8 import; Cmd+K find or create playlist.
- Tags (https://www.ora-dj.com/docs/library-management/tags): groups, coloured tags, key T opens sidebar, match "is any of / all of / none of".
- Search (https://www.ora-dj.com/docs/library-management/search): as-you-type, partial words, ranked; no operators.
- Analysis (https://www.ora-dj.com/docs/library-management/track-analysis): BPM and grid, key (standard and Camelot), waveform, energy 1-10 plus bar curve, vocal activity; all local; queue pauses while a deck plays; energy edits are locked against re-analysis.
- Track Prep editor (https://www.ora-dj.com/docs/library-management/track-prep): tap BPM, downbeat set and shift, 0.01 BPM fine, half/double, drag grid with 10 s Undo toast, check by ear with metronome and 5/25 ms nudges, 10 memory and 8 hot cues, "Auto Cue Points" at phrase starts and ends, Analyze scopes with a Lock.
- Phrases (https://www.ora-dj.com/docs/library-management/phrases): intro, build, breakdown, groove, transition, outro derived from energy; editable and survive re-analysis.
- Set planning (https://www.ora-dj.com/docs/set-planning): list, cover art, Canvas views; Groove Match suggests 9 tracks for a gap by "feel, energy and rhythm" (not tags or BPM), filters BPM/key/energy/year/genre/path; Hover Preview starts at the first groove.
- MCP access (https://www.ora-dj.com/docs/library-management/mcp-access): local agent control with Read only / 30 min / until off levels, cue edits previewed as a batch, exports limited to one folder.
- TrackID (online set tracklists, find owned tracks), Cloud Library (phone sync), recordings with track list export: outside our v1.

## Top 8 for fourfour
1. Compatibility report plus playlist-level "Prepare compatible copies" (FLAC/ALAC to AIFF or MP3 320, over 48 kHz flagged, originals untouched). M, Mo lane. Closes the FLAC gap seen on both competitors.
2. Library-wide duplicate cleanup with keeper, archive and restore; reuses the 2 s duration rule and our dup data. M, not Mo.
3. Verify and eject plus a visible "Recover sync" state after an interrupted write. M-L, Mo lane.
4. Moved-file reconnect by fingerprint with the copy-vs-move and two-candidates guards, plus folder Relocate. M, not Mo (relocate is decided).
5. Import-back from a stick ("Import it first"): cues, history, ratings come back, grid stays on the player. L, Mo lane. Ties to the open reverse-direction question.
6. Serato import (cues, loops, colours, grids, crates) and My Tags from Rekordbox. L, not Mo.
7. Smart (dynamic) playlists with AND/OR groups and a snapshot fallback, evaluated at sync time. M-L, small sync hook, Mo lane touch only for the export step.
8. Playlist export to M3U8, CSV, Traktor NML, Rekordbox XML, plus optional title suffix (key, BPM, energy) on the stick. S, the suffix is Mo lane.
Not ranked: PRO DJ LINK (L, Mo lane, needs hardware, defer); Groove Match style similarity (L, not Mo).
