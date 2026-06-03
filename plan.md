# Implementation Plan

## Goal
Add a `comment` field to the `Track` model in `models.rs` — but **the field already exists and is fully wired up** across the entire pipeline. No code changes are needed.

## Tasks

1. **Verify the field exists in `Track` and `ExistingTrack`**
   - File: `pioneer-usb-writer/src/models.rs`
   - Confirm `pub comment: String` is present on both `Track` (line 25) and `ExistingTrack` (line 160).
   - Acceptance: `grep -n 'pub comment' pioneer-usb-writer/src/models.rs` returns two matches.

2. **Verify the scanner populates it**
   - File: `pioneer-usb-writer/src/scanner.rs`
   - Confirm lines 161–163 read `ItemKey::Comment` from tags and pass it into the `Track` constructor (line 207).
   - Acceptance: `grep -n 'comment' pioneer-usb-writer/src/scanner.rs` shows the lofty lookup and struct field assignment.

3. **Verify the writers emit it**
   - Files: `pioneer-usb-writer/src/writer/pdb.rs` (lines 652–655), `pioneer-usb-writer/src/writer/onelibrary.rs` (line 760), `pioneer-usb-writer/src/writer/sync.rs` (line 99), `pioneer-usb-writer/src/reader/usb.rs` (lines 67, 109), `pioneer-usb-writer/src/reader/masterdb.rs` (lines 101, 133)
   - Confirm `track.comment` is written to PDB column 16, OneLibrary `djComment`, and compared in sync logic. Also confirm the reader paths deserialize it back.
   - Acceptance: `grep -rn 'comment' pioneer-usb-writer/src/` shows all usages are present and consistent.

## Files to Modify
None — the `comment` field is already implemented end-to-end.

## New Files
None.

## Dependencies
All three tasks are independent (read-only verification).

## Risks
- **Task is already done.** The `comment` field was added previously. If the intent was to add a *different* new field (e.g., `grouping`, `original_artist`), clarify which field is actually needed.
- **Frontend doesn't display comment.** `pioneer-test-ui/frontend/` has zero references to `comment`. If the goal includes surfacing comments in the test UI, that would be a separate task.
