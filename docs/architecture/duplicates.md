# Duplicate cleanup (Q60)

Finds copies of the same song in the library, merges each group into one keeper, and keeps what was merged away in an archive so one restore undoes it. No audio file is ever deleted or moved. Branch work, UI not yet reviewed.

## Detection (`pioneer-library/src/dedupe.rs`)

- Reuses the import-time rules in `duplicates.rs`: `normalise_tag`, the stored `artist_key`/`title_key`, and the 2 s duration rule (`DURATION_TOLERANCE_SECS`). Placeholder artists never match.
- Exact audio: same file size, then same content hash (std `DefaultHasher` over the whole file, compared within one run only) - byte-identical files, whatever their name, folder or tags. The library has no decoded-audio fingerprint, so a FLAC and an MP3 of one song are never "exact".
- Metadata only: same normalised artist and title, all durations within 2 s of the shortest.
- A group is `exact_audio` only when every member is in one exact class; anything mixed is `metadata_only`.
- `LocalLibrary::duplicate_rows()` is cheap and holds the library lock; `find_duplicate_groups(rows)` reads files and runs without it.

## Keeper proposal

Order: file present, lossless (flac, wav, aiff), higher bitrate, more playlists, more user cues, added first, lowest id. `reason` is one line naming the first rule where the keeper beats the runner-up.

## Merge (`dedupe_merge.rs`)

`apply_duplicate_merges(&[MergeRequest])`, one merge per group, failures reported per merge:

- Playlists: the keeper takes the earliest place of the removed copies in each playlist it was not already in.
- Cues: keeper's hot cue slots win; empty slots and memory cues (not within 50 ms of an existing one) are added. Only user or imported cues move, not analysis cues. Cue times are copied as stored (no encoder delay shift).
- Rating, colour, comment: only filled when the keeper has none. Colour goes to the keeper's `imported.json` without locking its grid or cues.
- `require_identical` (set for exact groups) compares the files byte for byte first and refuses the merge otherwise.
- Play counts do not exist in the library, so nothing moves for them.

## Archive and restore

- Additive schema, no version bump (`CREATE TABLE IF NOT EXISTS` on every open): `duplicate_merges` and `archived_tracks`. Each archived track is a JSON snapshot of its `tracks`, `analyses`, `artwork`, `analysis_failures`, `playlist_tracks` and `stick_tracks` rows. Cover files of archived tracks count as referenced (`sweep_covers` keeps them).
- The removed tracks' `anlz/<id>` folders move to `anlz/_archive/<id>`; the keeper's folder is copied to `anlz/_archive/keeper-<merge id>` before it changes.
- `restore_merge(merge_id)` re-inserts rows with their ids, moves the folders back, puts the playlist places back and takes the keeper's added places out. It refuses, changing nothing, when an id or a file path is used again. The keeper gets its pre-merge cues, rating, colour and comment back only if it has not changed since the merge.
- Stick memberships of removed tracks are restored too; the next sync (Mo's lane) decides what goes on a stick.

## Commands and UI

Tauri: `list_duplicate_groups`, `apply_duplicate_merges`, `list_duplicate_archive`, `restore_duplicate_merge` (`pioneer-test-ui/src/duplicate_commands.rs`).

UI: "Find duplicates..." in the library activity menu (top bar) opens `DuplicatesDialog` with a Duplicates tab (group, keeper, reason, Keep this one / Skip / Merge, "Accept all exact matches") and an Archive tab (Restore). Logic: `app/src/lib/duplicateCleanup.ts`, state: `app/src/stores/duplicate-cleanup.svelte.ts`.
