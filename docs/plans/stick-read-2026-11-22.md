# Open a Rekordbox stick: play and import, plan 2026-11-22

Plan only, nothing built. Goal (Merlin, 2026-11-22): "open a usb stick i synced with rekordbox itself and play the tracks and pull playlists into fourfour". Builds on the writer v2 stick model (branch `writer-v2`, `docs/architecture/stick-model.md` when it lands; plan in `docs/plans/writer-v2-2026-11-22.md` on `writer-v2-plan`). GPL is agreed, so the reader is the vendored `rbl-pdb` behind that model. No second PDB reader here.

## Where we are

| Piece | State |
|---|---|
| Stick view (`StickView.svelte`) | Shows library tracks the library remembers for the stick. Playlists made elsewhere ("only on this stick") show name and count from `read_usb_state`, no tracks. Since `stick-play-tab`, a stick track plays from its file on the stick |
| `read_usb_state` (`pioneer-usb-writer/src/reader/usb.rs`) | OneLibrary only. A stick with an empty `exportLibrary.db` and a full `export.pdb` (issue #41, the real stick copy at `~/dev/data/rbx-real-stick-2026-11-22`) reads as empty |
| Rekordbox import, USB source (`pioneer-library/src/rekordbox/usb.rs`) | Reads OneLibrary + ANLZ (cues, rating, colour, artwork) into `SourceLibrary`, then `import_rekordbox` writes tracks, playlists, cues and grids. Rejects export.pdb-only sticks. Imports the whole stick, no playlist choice. Track paths point at the stick |
| Player | Plays any absolute path (`playback_play`). Waveform and cues come from the library by track id, so a track that is not in the library has neither |

## What the app needs

1. **One read path.** `stick::read(root) -> StickModel` from writer v2 (export.pdb first, else OneLibrary, plus ANLZ). The app never opens either database itself. `read_usb_state` and `rekordbox/usb.rs` become thin adapters over the model, so #41 is fixed for sync, stick view and import at once.
2. **Stick listing command.** `read_stick(path) -> { playlists (tree, order), tracks (title, artist, bpm, key, time, usb_path, has_cues) }`, from the model. Off the main thread, cached per mount (a 1264-track stick should read in well under a second; measure).
3. **Stick view for foreign content.** "Only on this stick" playlists open into a track list like ours, rows from `read_stick` instead of `libraryTrackById`. Coordinate with the stick UI redesign (stick-ui-r1): same row component, a "not in library" mark.
4. **Play a stick-only track.** Load the player with the stick file path and a stick-scoped id (for example negative or `stick:<export_id>`), not a library id. Waveform, grid and cues from the model's ANLZ (PWV5/PWV7 colour wave, PQTZ grid, PCOB/PCO2 cues) through the decoder import already uses (`rekordbox/anlz.rs` `load_one`, which yields an `AnalysisResult` with grid, cues and colour detail). Needs one player change: analysis comes from the stick, not `get_analysis_data`.
5. **Import selected playlists.** `import_stick_playlists(path, playlist_ids, options)`: model -> `SourceLibrary` limited to those playlists and their tracks -> existing `load_analyses` + `import_rekordbox` (dedupe, cues, grids, rating, colour, artwork are already there). UI: select playlists in the stick view, "Add to library".
6. **Decide where imported audio lives** (Merlin): keep pointing at the stick (works only while plugged in, today's behaviour) or copy into a library folder (needs a target folder and space check). Recommendation: copy, because a library that points at a removable stick turns into missing files.

## Overlap with writer v2

| Need | Writer v2 slice | Note |
|---|---|---|
| export.pdb + OneLibrary reader, StickModel | 2 (`model.rs`, `read.rs`) | Same code; this plan only consumes it |
| ANLZ parse with cues, grid, colour wave | 1 (`rbl-anlz`) + 2 | Can replace `rekordbox/anlz.rs` later; not required for step 4 |
| #41 (foreign content invisible) | 5 (`plan.rs`) for sync | Read side is fixed by step 1 here as soon as slice 2 lands |
| Import back (cues, grids, ratings) | 8 | Steps 5 and 8 are the same feature; do it once, here, on the model |

## Effort and order

| Step | Effort (worker days) | Needs |
|---|---|---|
| 1 Adapters: `read_usb_state` + `rekordbox/usb.rs` over StickModel | 1 | writer v2 slice 2 |
| 2 `read_stick` command + tests on the real stick copy (counts only, no names committed) | 0.5 | 1 |
| 3 Stick view: foreign playlists open into tracks | 1 | 2, stick-ui-r1 landed |
| 4 Play stick-only tracks with stick ANLZ waveform/cues | 1.5 | 2 |
| 5 Import selected playlists | 1.5 | 2, decision 6 |
| Total | about 5.5 | |

## What can be built before the stick model lands

- Step 5's playlist filter on `SourceLibrary` (pure, testable today with the OneLibrary reader).
- The player change in step 4 (analysis from a stick-scoped source), tested with a OneLibrary stick written by our own writer.
- The stick view rows for foreign tracks, fed by today's `read_usb_state` (OneLibrary sticks already work; export.pdb-only sticks light up when step 1 lands).
- Tests that pin the target on the real stick copy: a model read must report the same playlist and track counts as `export.pdb` (counts only, no track names in the repo).

## Out of scope

Writing to a Rekordbox stick (writer v2 slices 5 and 6), history and My Tags import, master.db, PRO DJ LINK.
