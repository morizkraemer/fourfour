# Playlist export

Lists only, no audio. One playlist (or a folder, for Rekordbox XML and Traktor NML) goes to M3U8, CSV, plain text, Traktor NML or Rekordbox XML.
Code: `pioneer-library/src/playlist_export/`. Commands: `export_playlist`, `playlist_export_file_name`, `playlist_tracklist` in `pioneer-test-ui/src/playlist_export_commands.rs`. UI: sidebar playlist menu "Export playlist…" (format submenu, then the save dialog) and "Copy tracklist"; folder menu "Export folder…" (Traktor, Rekordbox only). Store: `app/src/stores/playlist-export.svelte.ts`.

`LocalLibrary::export_document(id)` gathers tracks with the effective BPM, key, grid, cues, rating and colour (user edits over imported over analysis), renders outside the library lock, and `ExportDocument::write` saves the file. `export_playlist` returns `{ written, missing, missing_examples }`: every track is written, tracks whose audio file is missing are still listed and counted in `missing`.

| Format | Content |
|---|---|
| M3U8 | `#EXTM3U`, `#EXTINF:<secs>,Artist - Title`, absolute path |
| CSV | `position,title,artist,album,bpm,key,duration,path`, RFC 4180 quoting, CRLF, duration as `m:ss` |
| Text | `Artist - Title` per line (title only without an artist, file name without tags) |
| Rekordbox XML | `COLLECTION` + `PLAYLISTS` (ROOT node), `TEMPO` markers, `POSITION_MARK` hot cues (Num 0-7) and memory cues (Num -1), loops (Type 4), rating (0-255), colour, `Location` as `file://localhost/...` percent-encoded |
| Traktor NML (v19) | `ENTRY` with LOCATION/ALBUM/INFO/TEMPO/MUSICAL_KEY, `AutoGrid` + `CUE_V2` hot cues, memory cues (HOTCUE -1) and loops, playlists as `PRIMARYKEY` entries |

Default file name: `<playlist> <YYYY-MM-DD>.<ext>` with `/\:*?"<>|` and control characters replaced by `_`. Files are UTF-8 without a byte order mark.

Beat grid to Rekordbox: one `TEMPO` marker while the beats keep tempo, spacing and bar count within 10 ms, a new marker where they stop. The XML importer rebuilds the same grid from the markers (`rekordbox/xml.rs`).

## Verified and not verified

- Rekordbox XML round trip through our own importer (`pioneer-library/tests/playlist_export.rs`): tracks, playlist order, tags, BPM, key, rating, colour, grid and cues (names, colours, loops) come back.
- Traktor NML: no sample NML is in the repo; structure follows the documented schema (version 19). Tested for well-formedness and content only, **untested in Traktor**. Assumptions: volume name `Macintosh HD` for paths outside `/Volumes/<name>/` and drive letters, a single constant grid (Traktor limit), Camelot to Traktor key mapping by circle of fifths, no cue colours.
- Rekordbox XML is untested in Rekordbox itself.
- Clipboard write uses `ClipboardItem` with a promise so WKWebView accepts it after the async fetch; checked in Chromium only, not in the Mac app.
