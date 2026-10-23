# Rekordbox import

Bring an existing Rekordbox library into the fourfour library: tracks and tags,
playlists, BPM, key, beat grids, hot and memory cues, ratings and colours.
Code: `pioneer-library/src/rekordbox/`. Commands: `rekordbox_import_preview`
and `rekordbox_import_run` in `pioneer-test-ui/src/main.rs`. UI: Settings →
General → "Import from Rekordbox" (`app/src/modules/RekordboxImport.svelte`).

Nothing is ever written to the source. `master.db` and the stick are opened
read-only.

## Flow

```
load_masterdb | load_usb | load_xml   →  SourceLibrary        (no library lock)
load_analyses(source, progress)       →  parsed ANLZ files    (no library lock, parallel)
LocalLibrary::rekordbox_preview       →  ImportPreview        (read-only)
LocalLibrary::import_rekordbox        →  RekordboxImportReport
```

`import_rekordbox_from` does the last two loads itself (used by tests). The
Tauri command parses ANLZ files before taking the library mutex, so the app
stays usable while a large library is read. Progress events:
`rekordbox-import-progress` with `{ stage, done, total }`; stages are
`reading`, `analysis`, `tracks`, `playlists`.

## Sources

| Source | Reader | Notes |
|---|---|---|
| Desktop `master.db` | own SQL in `rekordbox/masterdb.rs` (SQLCipher, read-only) | Needs the `share/` folder next to it (`PIONEER/USBANLZ`, `PIONEER/Artwork`) for grids and artwork. |
| Pioneer USB, OneLibrary | `reader::read_usb_state` for tracks and playlists, plus own queries on `exportLibrary.db` for folders, cues, rating, colour and ANLZ paths | A stick that only has `export.pdb` is rejected with a clear error: there is no PDB reader. |
| `rekordbox.xml` | own small XML reader in `rekordbox/xml.rs` (no new dependency) | Has no waveform, so grid and cues are kept as imported data until the track is analysed. |

The writer crate's `reader::masterdb::read_masterdb` is not used: it renumbers
track ids, drops folders, ratings, colours and the analysis path, and cannot
tell memory cues from hot cues with colours. `pioneer-usb-writer/src` is
untouched.

## Field mapping

| Rekordbox | fourfour |
|---|---|
| Path (`FolderPath`, `path`, XML `Location`) | `tracks.source_path` after remap; `usb_path` generated as `/Contents/<artist>/<file>` (same rule as the scanner) |
| Title, artist, album, genre, label, remixer, comment, year, disc, track no. | same-named `tracks` columns |
| BPM (×100) | `tracks.tempo`; `analyses.bpm` |
| Key (`djmdKey.ScaleName`, OneLibrary key table, XML `Tonality`) | `tracks.key` (Camelot) |
| Length, sample rate, bit rate, file size | same-named columns |
| Beat grid (ANLZ `PQTZ`; XML `TEMPO`) | ANLZ file in the library (`anlz/<id>/ANLZ0000.DAT`), plus waveform and colour waveform from the same ANLZ |
| Hot and memory cues (`djmdCue`, OneLibrary `cue`, ANLZ `PCO2`/`PCOB`, XML `POSITION_MARK`) | ANLZ cue points (`hot_cue_number` 0 = memory, 1 = A, ...), loops keep their end; name and colour in `imported.json` |
| Rating | `imported.json` (`rating`, 0–5; values above 5 are divided by 51) |
| Colour label | `imported.json` (`color`: name from `djmdColor`, or `#rrggbb` from XML) |
| Artwork | library artwork table (from `share/PIONEER/Artwork` or the stick) |
| Playlists | library playlists; folders flattened into `Folder / Sub / Playlist` |

Cue source order: database cues, then the ANLZ `.EXT` `PCO2` cues (with name
and RGB colour), then whatever the writer crate's ANLZ reader found.

Cue colours from `master.db` and OneLibrary are stored as the palette index
(`"table:N"`); only ANLZ `PCO2` and XML carry RGB. The library's ANLZ cue
writer has no colour or name, so colours exist only in `imported.json`.

Not imported: smart playlists (counted as `skipped_smart_playlists`), My Tags,
play history, sampler slots, related tracks, hot-cue banks, fade/load markers
from XML.

## Path remap

Rekordbox stores absolute paths from the machine it ran on.

- `detect_common_root(paths)` returns the longest shared directory, compared
  per path component, case-insensitively, `/` and `\` treated alike.
- `PathRemap { from_prefix, to_prefix }.apply(path)` swaps the prefix,
  case-insensitively. `ImportOptions.remap_from` defaults to the detected
  root; `remap_to` is the caller's replacement root. Without `remap_to`, paths
  are used as they are, or the source's implied remap: for `master.db` the
  `djmdProperty` `BaseDBDrive` → `CurrentDBDrive` swap when both are set (idea
  from rbxport's design; no code taken), for a USB stick the stick root.
- `CaseInsensitiveResolver` finds the real file on case-sensitive disks by
  matching each path component against the directory listing (cached).
- The preview reports `files_found`, `files_missing` and up to ten missing
  examples. Tracks whose file is missing are skipped unless
  `include_missing` is set.

When a library spans several roots (see the test below) the detected root is
only their common parent. Pass `remap_from` explicitly.

## Merge policy

Same path (ignoring case) or same file name and file size counts as "already in
the library".

- Existing track: the user's values are kept; only empty fields are filled
  (empty text, 0 numbers). Never overwritten, never deleted.
- Existing analysis (grid, cues, waveform): kept as is. The imported one is
  used only when the track has none.
- Existing playlist with the same name: imported tracks that are not in it yet
  are appended. Nothing is removed or reordered.
- A second import of the same source changes nothing.
- Duplicate rows inside the source that point to one file collapse into one
  library track (reported under `tracks_merged`).

## Imported data survives re-analysis

Per track the library keeps `anlz/<id>/imported.json` (`ImportedMeta`): source,
rating, colour, cue names and colours, and the lock flags `grid_locked` and
`cues_locked`. `LocalLibrary::set_analysis` consults it: a locked beat grid
(and its tempo) is kept as stored, locked cues are restored from the file, so
the analysis pipeline can re-run without destroying them.
`release_imported_locks(track_id)` hands the track back to the analyser. XML
imports keep the grid in the file until the first analysis writes it. This
follows cratedrop's idea of protected manual data; no library DB schema change
was needed (that needs Merlin's sign-off).

## Preview and report

`ImportPreview`: tracks, new tracks, conflicts, playlists, folders, smart
playlists skipped, hot and memory cues, tracks with a beat grid, detected root,
applied remap, files found and missing (+ examples), playlist name conflicts.
`RekordboxImportReport`: tracks imported / merged / skipped as missing, files
found / missing, playlists imported / merged, grids, hot and memory cues,
`cues_without_analysis`.

## Tests

- `pioneer-library/tests/rekordbox_import.rs`: synthetic encrypted `master.db`
  (tags, rating, colour, cues with colours, loop, folder + smart playlist, case-
  different file names, real ANLZ written by the writer), a USB stick written by
  `write_usb` and read back, an XML export; merge policy, re-analysis lock,
  `include_missing`, caller prefix, idempotent re-import.
- `rekordbox/{remap,xml,anlz}.rs` unit tests (PCO2 entries of varying length
  with names and colours, remap, XML parsing).
- `pioneer-library/tests/rekordbox_real_library.rs`: `#[ignore]`d, needs
  `FOURFOUR_RB_LIBRARY`, `FOURFOUR_RB_AUDIO`, `FOURFOUR_RB_STRIP`. Plain
  `cargo test --workspace` (and so CI) skips it; run with `-- --ignored
  --nocapture`. Prints counts only and works in a temp library.

### Result on Merlin's library (counts only)

`master.db` + `share/`, audio root remapped with the strip prefix
`/Users/merlinkraemer/local/2-Areas/1-Mukke/`:

| | |
|---|---|
| tracks in master.db | 1382 |
| playlists | 4 (0 folders, 0 smart); 3 imported, 1 skipped (all its tracks missing) |
| files found / missing | 631 / 751 |
| missing: outside the strip prefix | 730 (698 under a Mac dev path, 30 sampler presets, 2 demo tracks) |
| missing: under the prefix, not in the test data | 21 |
| tracks with an ANLZ beat grid file | 1374 |
| tracks imported | 615 (+ 16 merged: duplicate rows for the same file) |
| analyses stored / grids imported | 608 / 607 |
| playlist entries | 641 |
| hot / memory cues | 0 / 0 (`djmdCue` is empty and no ANLZ `PCO2` entries exist) |
| second import | 0 new, 631 merged, 3 playlists merged |

Run time about 40 s in a debug build for the import itself.

## What is missing

- `export.pdb`-only sticks (no reader in the writer crate).
- RGB for database cue colours (only the palette index is kept) and for hot
  cues in the library's own ANLZ output.
- Ratings and colour labels are stored but not shown in the UI yet.
- Only one remap prefix per run; libraries spread over several roots need one
  run per root (Merlin's needs two more for the 698 + 32 tracks).
- File names with different Unicode normalisation (macOS NFD vs NFC) are not
  matched.
- Imported tracks count as analysed, so the app will not queue them for
  analysis by itself.
- Cues without an ANLZ analysis stay in `imported.json` until the track is
  analysed.
- Smart playlists, My Tags, history.
- The XML reader is a small tolerant tokenizer: no DTD, and `>` inside an
  attribute value is not supported.

## For the format library (`pioneer-usb-writer`, not changed here)

1. `writer/anlz.rs` lines ~258, 314, 364, 404, 448: `i * src_len / entry_count`
   in `u32` overflows when a colour-waveform detail has more than about 65k
   entries (tracks over roughly 7 minutes at 150 entries per second). It
   panics in debug builds and indexes garbage in release. The importer
   decimates the detail as a workaround (`fit_color_detail_for_writer`);
   `u64` arithmetic in the writer would remove it.
2. `reader/anlz.rs` `parse_pco2` assumes 88-byte entries; real `PCP2` entries
   have a `len_entry` field and grow with the comment. It also drops name and
   colour. `rekordbox/anlz.rs::read_ext_cues` is a wrapper that does it
   properly.
3. `reader/masterdb.rs` and `reader/usb.rs` keep their SQLCipher keys private;
   the importer repeats them. Exposing `open_masterdb`/`open_onelibrary` would
   remove the duplicates. `read_usb_state` also skips folders, ratings, colours
   and cues, hence the extra queries.
4. A PDB reader would make older sticks importable.
