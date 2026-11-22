# Stick model and reader (writer v2)

**Status:** experiment on branch `writer-v2`. The old writer (`writer/filesystem.rs`, `sync_usb`) is still the default; nothing in the app calls v2 yet. Plan: `docs/plans/writer-v2-2026-11-22.md`.

**Code:** `pioneer-usb-writer/src/stick/`, built on the vendored `vendor/rbl-pdb` and `vendor/rbl-anlz` (from rbxport, GPL; see `NOTICE`).

One model describes what is on a stick, whoever wrote it. Reading a stick gives the model; writing a stick starts from the model. The dump tool prints it.

```
stick root ──read──> Stick { device_library, one_library }   each an Option<StickModel>
                          │ library()  = the fuller one
                          v
                     StickModel ──emit──> export.pdb, exportLibrary.db, ANLZ, artwork
```

## Reading a stick

```rust
use pioneer_usb_writer::stick::{Stick, LibraryKind};

let stick = Stick::read(Path::new("/Volumes/MK_BLACK"))?;
if !stick.has_library() { /* no PIONEER/rekordbox folder: blank stick */ }

let library = stick.library().unwrap();          // the database that holds more tracks
for playlist in library.children(0) {            // root level, in display order
    for track in library.playlist_tracks(playlist.id) {
        let artist = library.artist_name(track.artist_id);
        let audio = stick.audio_path(track);     // absolute path of the audio file
    }
}
let analysis = stick.read_analysis(track)?;      // lazy: reads that track's DAT/EXT/2EX
analysis.beats(); analysis.cues(); analysis.has_phrases();
```

- `Stick::read` never writes to the stick. `exportLibrary.db` (and its `-wal`/`-shm`) is copied to a temp folder and opened there.
- A stick has up to two databases. `stick.device_library` is `export.pdb` (all players), `stick.one_library` is `exportLibrary.db` (newer players). Rekordbox often fills only one: the real stick checked on 2026-11-22 has 1264 tracks in `export.pdb` and 0 in `exportLibrary.db`.
- `stick.library()` picks the one with more tracks, `export.pdb` on a tie. `stick.primary_kind()` says which. **Use this, not `reader::read_usb_state`**, which only looks at OneLibrary (issue #41).
- Root folder `PIONEER` or `.PIONEER` are both accepted.
- Analysis is not loaded by `Stick::read` (a full stick has thousands of files). Call `stick.read_analysis(track)` per track. It returns the parsed files (`rbl_anlz::Anlz`) with every section kept, so nothing is lost when they are written back.

## The model

`StickModel` (in `model.rs`) is plain data, no I/O:

| Field | What |
|---|---|
| `device` | Stick name, database version, creation date, background colour, track count |
| `tracks: Vec<StickTrack>` | One per track, in the order stored |
| `artists`, `genres`, `labels`, `keys`, `colors`, `artworks: Vec<StickName>` | Lookup tables: `id` + `name` (for artwork the name is the path of the small image) |
| `albums: Vec<StickAlbum>` | `id`, `name`, `artist_id` (album artist) |
| `playlists: Vec<StickPlaylist>` | Folders and playlists as flat nodes: `id`, `parent_id` (0 = root), `sort_order` (0-based among siblings), `is_folder`, `track_ids` in order |
| `histories: Vec<StickHistory>` | Play history lists written by players |
| `menu` | Browse-menu rows of `export.pdb`, kept as raw bytes |

`StickTrack` holds ids, not names: `artist_id`, `album_id`, `genre_id`, `label_id`, `key_id`, `remixer_id`, `composer_id`, `original_artist_id`, `artwork_id`, `color_id`. Resolve them with `model.artist_name(id)`, `album_name`, `genre_name`, `label_name`, `key_name`, `color_name`, `artwork_path`. An id of 0 means "none" and resolves to `""`.

Other track fields: `id` (the stick's own track id, stable across syncs), `title`, `path` (`/Contents/...`, as stored), `filename`, `tempo_x100`, `duration_secs`, `year`, `track_number`, `disc_number`, `bitrate`, `sample_rate`, `sample_depth`, `file_size`, `file_type` (1 MP3, 4 M4A, 5 FLAC, 11 WAV, 12 AIFF), `rating` (0 to 5), `play_count`, `comment`, `mix_name`, `isrc`, `date_added`, `date_created`, `release_date`, `analyze_path`, `hot_cue_auto_load`, `master_db_id`, `master_content_id`, `content_link`.

Per-database extras:

- `cues: Vec<StickCue>` is filled only from OneLibrary's cue table. For an `export.pdb` stick, cues live in the analysis files: use `stick.read_analysis(track)?.cues()`.
- `analysed_bits` exists only in OneLibrary.
- `device_library_strings` keeps the `export.pdb` text slots we do not understand, byte for byte, so they survive a round trip.

Helpers: `track(id)`, `track_by_path(path)`, `playlist(id)`, `children(parent_id)`, `playlist_tracks(id)`; for building a model `intern_artist/genre/label/key/album/artwork(name)` return the existing id or add a row.

Ids: names are interned exactly as written (case matters), so "test artist" and "Test Artist" are two artists. This is on purpose: the old writer merged them in one database and not in the diff (issue #42).

### Unverified points

- `date_added` is text slot 15 and `date_created` slot 10 of the track row (rbxport's reading). The Kaitai spec names them the other way round. Merlin's export with both databases filled will settle it.
- OneLibrary `playlist.sequenceNo` is read as 1-based and stored 0-based in `sort_order`.
- History rows in `export.pdb` are read by name and entries only.

## Writing (fresh export)

```rust
use pioneer_usb_writer::stick::emit;

emit::write_usb_v2(root, &tracks, &analyses, &playlists, device)?;   // same inputs as write_usb
emit::copy_stick(&source_stick, LibraryKind::DeviceLibrary, root, false)?;  // round trip of a read stick
emit::write_fresh(root, &FreshExport { model: &model, files })?;     // the general form
```

- `export.pdb` is built by `device_library::build(&model)` on `rbl-pdb` (index pages, the 255-row rule, sequence numbers). Rows are encoded from the model in `device_library::encode_tables`.
- `exportLibrary.db` is written by `one_library::write`, which uses the schema and menu rows of the old writer and takes every id from the model, so both databases agree.
- Analysis files are either carried (`AnalysisSource::Carry`: parsed and written back section by section with the stick path set) or authored by the old `writer::anlz` from our own analysis.
- `convert::model_from_tracks` is the one place that turns the old writer inputs (`Track`, `AnalysisResult`, `Playlist`) into a model. It keeps the header values the old writer proved on a CDJ-3000.
- The creation date comes from the caller (`StickDevice.created_date`), so the same input gives the same `export.pdb` bytes.

Not done yet: staged publish and recovery, `exportExt.pdb`. See the plan.

## Incremental sync (`plan.rs`)

```rust
use pioneer_usb_writer::stick::plan::{plan, apply, Ownership, Wanted};

let stick = Stick::read(root)?;
let wanted = Wanted { tracks: &tracks, analyses: &analyses, playlists: &playlists };
let planned = plan(&stick, wanted, &owned, device)?;     // reads only; planned.report(), planned.is_noop()
let report = apply(root, &planned, wanted)?;             // writes; then store planned.ownership
```

`pioneer_library::LocalLibrary::sync_usb_v2(root, name)` does this for the whole library and keeps the ownership in `PIONEER/fourfour-manifest.json` (new optional `v2` field). `multi_sync` still calls the old writer.

`Ownership` is what fourfour wrote on this stick: track paths, playlist ids, and a fingerprint per track (analysis + cover). Everything else on the stick is **foreign** and is kept:

- Foreign tracks stay in **both** databases with their ids and rows; their audio, analysis and cover files are never written or deleted. A Rekordbox stick with a full `export.pdb` and an empty OneLibrary gets a OneLibrary holding all tracks, theirs and ours (issue #41, write side). Cues of foreign tracks are filled into OneLibrary from the analysis files.
- Foreign playlists and play histories stay, in their order; ours are placed after them.
- If both databases have tracks and one has tracks the other lacks, `plan` refuses with a clear error and nothing is written.
- Syncing nothing onto a stick we own nothing on writes nothing.
- A wanted track whose path is already there as a foreign file is linked into our playlists and otherwise left alone (`AdoptedForeign`): not copied, not owned, never deleted.

Rules for our own tracks:

- Identity is the stick path. A matched track keeps its id, dates, rating and every field we do not set; a new track gets `highest id + 1` (ids of removed tracks are not reused). Our playlists keep their id by name and parent.
- `Replace` when the file size differs, `Update` when the fingerprint or any row field differs (analysis and cover are rewritten, audio is not copied), else `Unchanged`. A changed cue or grid is therefore an `Update`. A stick taken over from the old sync has no fingerprints, so every track is updated once.
- Names are compared exactly, in the databases and in the diff, so two spellings of an artist are two artists and nothing "updates" forever (issue #42).
- Paths are made safe in one place (`paths.rs`): trailing dots and spaces are cut from every folder and file name, `? : * " < > | \` and control characters become `_`, Windows device names get a `_` in front, names are cut to 255 UTF-16 units keeping the extension, and two paths that differ only in letter case get ` (2)`, ` (3)` on the file name. The artist tag itself is not changed.
- Removing: only our audio file, our `ANLZ0000.DAT/.EXT/.2EX` and our cover files; folders only when empty. Artist, album, genre and label rows go only when our removed tracks were their last users.
- Order of writes: new files, then both databases, then deletions. There is no staging yet: a pulled stick can still leave a half-written database (slice 6).
- The creation date of a stick is kept once set, and a track keeps its added date, so the databases do not change from day to day.

Decision taken here, for Merlin to confirm: on a stick whose OneLibrary Rekordbox left empty, v2 fills OneLibrary with all tracks rather than only ours, so both databases always show the same stick.

Tests: `pioneer-usb-writer/tests/stick_plan.rs` (12), `pioneer-library/tests/stick_sync_v2.rs` (3: fresh, takeover of an old-sync stick, Rekordbox-like stick), unit tests in `paths.rs`. Local with the real stick copy: adding 3 tracks gives 1267 in both databases, all 1264 foreign rows byte-identical, a second plan is a no-op.

## Dump and diff

```bash
cargo run -p pioneer-usb-writer --example stick_dump -- <stick root> [--raw] [--pages] [--no-normalise]
cargo run -p pioneer-usb-writer --example stick_diff -- <stick a> <stick b> [--raw] [--pages] [--no-normalise]
```

`stick_dump` prints one fact per line in a fixed order; `stick_diff` prints the lines that differ and exits with 1 if there are any (0 = equal, 2 = error).

| Line starts with | Content |
|---|---|
| `pdb ...` | `export.pdb`: header, row count per table, the model (`track`, `artist`, `album`, `playlist`, ...); with `--raw` every row as hex, keyed by id; with `--pages` every page header |
| `ext ...` | `exportExt.pdb`: rows per table |
| `onelib ...` | `exportLibrary.db`: the model; with `--raw` the schema and every row of every table |
| `anlz <file> ...` | One line per file and one per section: tag, header length, total length, hash |
| `file ...`, `audio ...` | Other files under `PIONEER` (size + hash), audio files (size) |
| `not-compared ...` | Files left out on purpose: `RBFLTR.DAT`, `USBMNG.DAT`, `-wal`/`-shm`, `._*` |

Normalising (on by default) hides what differs between two exports of the same thing: the creation date, page numbers and sequence numbers, and the per-page slot number in bytes 2-3 of track, artist, album and property rows. `--no-normalise` shows them.

A dump of a real stick contains track names. Do not commit dumps of private sticks.

## Tests

- `pioneer-usb-writer/tests/stick_read.rs` (CI, 11 tests): the old writer's stick read through the new reader from both databases, per-track analysis, an empty OneLibrary next to a full `export.pdb` (the #41 case), OneLibrary only, `.PIONEER`, a blank folder, a broken `export.pdb` (an error, never "empty"), reading changes no file, two exports dump the same and one changed rating shows as one changed track.
- `pioneer-usb-writer/tests/stick_model.rs` (CI, 13 tests): v2 export from the old writer inputs, read back from both databases; same input gives the same `export.pdb` bytes; only dated lines differ between two days; a stick written from a read stick dumps the same (`export.pdb` byte-identical); v2 rows compared with the old writer's rows as a ratchet.
- `pioneer-library/tests/export_smoke.rs`: `LocalLibrary::write_usb_v2` holds the same library as `write_usb`.
- `pioneer-usb-writer/tests/stick_real.rs` (local only, 4 tests): runs when `FOURFOUR_REAL_STICK` points at a Rekordbox stick copy, otherwise passes without doing anything. Prints counts only, never names.

```bash
FOURFOUR_REAL_STICK=~/dev/data/rbx-real-stick-2026-11-22 cargo test -p pioneer-usb-writer --test stick_real -- --nocapture
```

## What the checks found (2026-11-22)

Against the real Rekordbox stick copy (1264 tracks, metadata only, no fixture from Merlin yet):

- All 3792 analysis files (`.DAT`, `.EXT`, `.2EX`) come out byte-identical after parse and write, PSSI, PQT2 and PVB2 included.
- Every `export.pdb` row of every table is byte-identical after decode and encode, except 2 of 1264 track rows. In those two Rekordbox did not pad before a long string; everywhere else it pads to 4 bytes, as we do. Cause unknown (possibly an older Rekordbox wrote them).
- Rekordbox leaves stray bytes in that padding; we write zeros. The dump clears padding before comparing.
- A full copy of the stick through the v2 writer reads back as an equal model.
- ISRC is stored as a `0x90` string holding `0x03`, the ASCII code and a zero byte, not as a plain string. rbxport writes it plain; we follow Rekordbox.
- Page layout (page count, rows per page, sequence numbers) is not compared yet: Rekordbox's file has deleted rows and 501 pages from many syncs. That needs Merlin's fresh export.

Against the old writer (same input, `stick_model.rs` ratchet), rows differ in 8 tables, all explained:

| Table | Old writer | v2 | Rekordbox stick says |
|---|---|---|---|
| tracks | long strings not padded | padded to 4 bytes | padded (2 exceptions) |
| genres, keys | **no rows at all** when they fit one page: the page is never written, so a player reading `export.pdb` gets no genre and no key names | rows written | rows present |
| colors | byte 4 = 0 | byte 4 = colour id | byte 4 = id |
| playlist_tree | `sort_order` counts from 1 across all nodes | from 0 per parent | matches v2 |
| playlist_entries | entry index runs on across playlists | starts at 1 in each playlist | matches v2 |
| sorts | one row differs from rbxport's captured default | rbxport default | matches the old writer (it is a per-user setting) |
| property | track count 6 for 3 tracks, no stick name | real count, stick name | real count |

Also seen: the old `export.pdb` takes the tempo from the track row and OneLibrary from the analysis, so they can disagree; v2 uses the analysis BPM in both.

Artist, album, label and artwork rows are byte-identical between the old writer and v2.
