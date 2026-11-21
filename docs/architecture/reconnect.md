# Moved files: reconnect and relocate

Code: `pioneer-library/src/reconnect.rs`, commands in `pioneer-test-ui/src/reconnect_commands.rs`, UI in
`app/src/modules/ReconnectPanel.svelte` (inside the "Library check" dialog, under the missing-files line).

## Identity

The library had no fingerprint; duplicate detection uses path, file name + size, or artist + title + duration
(`duplicates.rs`). Reconnect adds a **content hash**: FNV-1a over the file size and three 32 KiB samples at 25, 50
and 75 % of the file (whole file when under 96 KiB). It is stored per track in `track_fingerprints`, taken while the
file is there: `spawn_check` runs `backfill_fingerprints` after every launch check, in chunks, without holding the
library lock while files are read. A reconnect also stores the hash of the new file.

Schema: two additive `CREATE TABLE IF NOT EXISTS` blocks (`RECONNECT_DDL`, no version bump, same pattern as
`pending_artwork`): `track_fingerprints`, and `folder_relocations` + `folder_relocation_tracks` (the undo record).

## Find moved files

The user picks a folder to search (the library stores no watched folders). For each missing track (file not found,
not "unreadable"):

1. Cheap: candidate files in the folder that no track uses, same size, duration within 2 s.
2. Confirm: sampled hash equals the stored hash. Without a stored hash the candidate is only "unconfirmed" and needs
   a choice (tags must not contradict; an untagged file always passes, because a rename changes its tag-derived title).

Outcome per track: **reconnected** (exactly one confirmed candidate), **needs a choice** (several candidates;
unconfirmed; the file is also wanted by another missing track; the file is a copy of a track whose file still
exists), or **still missing**. `apply_reconnects` refuses a track whose old file still exists (copy, not move) and a
path another track uses. The track id never changes, so analysis, cues, edits and playlists stay.

Weak spot (same as Ora's): a file whose tags were rewritten *and* whose size changed before the hash was stored
is not recognised.

## Relocate folder

The user gives the old root (prefilled with the folder all missing tracks share; the old folder does not exist, so a
picker cannot choose it) and picks the new root. Tracks under the old root map by relative path. The preview counts
found (file exists, path free) and still missing. Apply is one transaction and writes the old and new paths to
`folder_relocations`; undo puts back every track still at its new path whose old path is free (others are
skipped and counted) and deletes the record. `last_folder_relocation` gives the newest undoable step.
