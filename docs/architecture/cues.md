# Cues

Hot cues (A to H), memory cues, the CUE button, and how edits to cues and the beat grid are kept.
Branch `wd-hotcues` (from `q41-feedback`). Written for Merlin's report F9b ("the whole hot cue system doesn't work")
and F25 (chips truncated to "H..").

## What was broken

1. No way to set a cue. The inspector's "+" chip had no handler, the players had no pad or key, the only
   writer of cues was the context-menu "Set test cues" (three fixed cues, `set_test_cues`).
2. Hot cue and memory cue were not told apart in the app. Chips were named from `hot_cue_number` only in one place,
   and every cue got a colour from its list index (`CUE_COLORS[i % 8]`), not from the cue.
3. F25: a chip is a flex row with a name that shrinks to 12 px; the name `Hot Cue H` / `Memory Cue` was cut to "H.." and
   "M.". The strip had no room for the slot letter, and chips had `flex: 1 1 33%` with a 72 px minimum, so 3 per row
   overflowed the inspector.
4. The player and the inspector each built their own cue list from the analysis payload (`PlayerTrack.cues`,
   library `track.cues`), so nothing could stay in sync after an edit. Library rows carried a placeholder
   (`Cues Injected`) from `has_cues`, which the backend always sends as false.
5. The CUE button always played from `cues[0]` (the first cue of any kind, or 0) while held and returned there. It never
   set a cue point, and "pressed while playing" did not return to the cue point.
6. The waveform display took cues only with a full `setData` (which resets zoom), keyed on the cue count, so a rename
   or recolour would never have redrawn.
7. The payload and the cue list carried no name or colour (`get_analysis_data` sent `hot_cue_number`, `time_ms`,
   `loop_time_ms`); imported names and colours lived only in `imported.json`.
8. Nothing was stored. There was no command to write cues. Re-analysis (`set_analysis`) replaced the ANLZ cues; only
   imported tracks (`imported.json` with `cues_locked`) kept them. The beat grid had the same gap: the wc-player
   worker noted that re-analysis replaces a user grid edit on non-imported tracks.
9. No shortcuts for any of this.

## Model

`Cue` (app `lib/cues.ts`) and `ImportedCue` (library, serde):

| field | meaning |
|---|---|
| `slot` / `hot_cue_number` | 0 = memory cue, 1 to 8 = hot cue A to H |
| `timeMs` / `time_ms` | position |
| `loopMs` / `loop_time_ms` | loop end, `null` = plain cue (no UI to make loops yet; they survive and are drawn as cues) |
| `color` | `#rrggbb`, or `table:N` for a palette index of an imported Rekordbox cue, `null` = default |
| `name` | free text, at most 32 characters, empty = none |

A hot cue is keyed by its slot, a memory cue by its time. Memory cues within 50 ms are the same cue. A slot holds one
hot cue; "first free" fills gaps. Validation lives in `pioneer_library::edits::normalize_cues` (slot 0 to 8, no
duplicate slot, no two memory cues at one time, loop end after start) and is the same in `lib/cues.ts`.

Colour: the Pioneer-style 16-colour palette (`CUE_PALETTE`, pink, red, orange, yellow, lime, green, mint, aqua, sky,
blue, indigo, purple, magenta, rose, peach, white). The hex values are an approximation of Rekordbox's palette, not
read from hardware. `table:N` (imported) shows as palette entry N. Defaults: each hot slot has its own colour, memory
cues are red.

## Storage

No DB schema change. Per track, next to the ANLZ files in `anlz/<id>/`:

- `ANLZ0000.DAT/.EXT/.2EX`: cue positions and loops (hot cues `PCOB` type 1 / `PCO2`, memory cues type 0). This is
  what USB export reads (`get_analysis`).
- `edits.json` (`UserEdits`): `grid_locked`, `cues_locked`, and the full cue list with names and colours.
- `imported.json` (`ImportedMeta`, Rekordbox import): unchanged. Its cue names and colours show until the user edits.

`LocalLibrary::get_cues` order: user list (if `cues_locked`), else imported list (if its `cues_locked`), else the cues
in the ANLZ file (no names or colours). `set_cues` validates, writes `edits.json`, then rewrites the ANLZ files when
the track has an analysis. `set_beat_grid` now also sets `grid_locked`. `get_analysis_data` sends cues from
`get_cues` with `name` and `color`.

## The lock

`keep_imported_data` (called by `set_analysis` and `set_analysis_stages`) keeps the stored grid and tempo when
`edits.grid_locked` or the imported `grid_locked` is set, and restores locked cues (user list first, then imported).
So background re-analysis, "Analyze selection" and the Rekordbox merge path leave edited data alone.
`release_user_edits(track_id)` hands a track back to the analyser (the imported counterpart is
`release_imported_locks`). There is no UI for it yet; a "Reset grid and cues" action is the obvious one.

Tests (`pioneer-library`, `pioneer-test-ui`): cue CRUD with name, colour, loop and reopen; rejected lists change
nothing; user edits survive re-analysis and are released again; an untouched track is replaced by re-analysis;
imported cues show names until edited and the edit then wins over the import; payload carries name and colour.

## App

- `stores/cues.svelte.ts`: one store of cues per track (`$state.raw`), read by the inspector and both players. An
  edit shows at once and is saved after 400 ms (shared `lib/persister.ts`, the same debouncer as the grid:
  newest wins, writes are serialised, an edit of another track writes the waiting one first, flushed before a track
  loads). An analysis payload does not overwrite a track with an unsaved edit.
- `stores/cue-actions.ts`: set at playhead, trigger, jump, rename, recolour, delete.
- Inspector (`DetailPane`, `CueEditor` in the design system): HOT CUE and MEMORY CUE groups of chips in a wrapping
  grid (min 104 px per chip). A chip shows the slot letter on the cue colour, the name if set, the time. Click jumps
  (loaded track only), the `···` button (or double click) opens the editor: name, 16 swatches, delete. `+` adds at the
  playhead and is disabled unless the selected track is the loaded one (same rule as the strip seek).
- Big player: pads A to H (filled with the cue colour when set; click sets or jumps and plays; right click edits),
  `M+` for a memory cue, markers with the slot letter on the zoom view, lines on the overview, dots on the compact strip.
- CUE button (`lib/cueButton.ts`, a pure state machine): paused, press sets the cue point at the playhead and plays
  while held, release returns to the cue point and pauses; playing, press returns to the cue point and pauses;
  pressing play while CUE is held keeps playing. The cue point on load is the first memory cue, else the start. It is
  not saved (a memory cue is the saved version).

## Shortcuts

| keys | action |
|---|---|
| `C` | CUE as a tap: playing returns to the cue point, paused sets it at the playhead (keyboard has no hold) |
| `M` | add memory cue at the playhead |
| `Shift+1` to `Shift+8` | hot cue A to H: set at the playhead if empty, else jump there and play |
| `Alt+Shift+1` to `Alt+Shift+8` | delete hot cue A to H |

All need a loaded track, appear under "Cues" in settings, and can be rebound. Plain `1` to `9` stay the favourites.
Bindings use the key `code`, so any keyboard layout works.

## What USB export does with them

`write_usb` and the multi-stick sync read cues through `get_analysis`, so edits and imported cues reach the stick as
positions and loops: `PCOB` (DAT) and `PCO2` (EXT), and the OneLibrary `cue` table (`kind` = slot, `inUsec`, `outUsec`).
Names and colours do not reach the stick (see below). Locked grids export as stored.

## Writer issues

Items 1–3 fixed on branch wu-writer (2026-10-27): names, colours, loop type and PCPT status are written; see cdj-checklist-2026-10.md for the CDJ check. Items 4–6 are open.

1. Cue names and colours are dropped. `PCP2` writes 58 zero bytes after `loop_denominator` where Rekordbox keeps
   colour code, RGB and the comment. The OneLibrary `cue` row is written with `colorTableIndex = 0` and
   `cueComment = ''`. `CuePoint` has no name or colour field, so the writer cannot carry what the app now stores.
   A `name` and `color` on `CuePoint` (and the UI side is ready to fill them) is the change that is needed.
2. Loops: the `PCPT`/`PCP2` entry type byte is always `0x01` (cue), also when `loop_time_ms` is set. Rekordbox writes a
   different type for loops. The OneLibrary row sets `isActiveLoop`, the ANLZ entry does not say loop.
3. `PCPT` status is written as 0 (Rekordbox uses a non-zero status for an active cue). Not proven on a CDJ either way.
4. The DAT `PCOB` hot-cue section can carry only the first hot cues on older players (A to C on some models); slots
   D to H live only in `PCO2`. The reader prefers `PCO2` when present, so app round trips are fine; older hardware may
   show fewer hot cues. Needs a hardware check.
5. Memory cue `PCOB` sentinel is `0` when entries exist, `0xFFFFFFFF` for hot cues. Kept as found; unverified for
   many memory cues.
6. `hot_cue_number` above 8 is not produced by the app (validated), but the writer would write it as is.

## Not done

- Loops cannot be made in the UI (data model and storage keep them).
- Beat-grid quantise on cue set, cue snapping, and hold-to-preview for hot cue pads (a pad press jumps and plays).
- Dragging a marker, and clicking a marker on the waveform to jump.
- Reset action for the user-edit lock.
- The vanilla canvas `detail-pane.js` still draws the old plain chips (prototyping canvas only).
- Browser-mode mocks keep cues in `localStorage` (`fourfour.mock.cues`); the real store is `edits.json`, only the
  Rust side is tested against it. Not tried in the Tauri window or on a CDJ.

## Verification

`cargo test --workspace` (library cue tests, payload test), `npm test` (cue model, CUE state machine, store, chip
labels, shortcuts), and `app/perf/cues-walkthrough.mjs`: headless Chromium against the Vite dev server in mock mode.
It sets, jumps, renames, recolours and deletes hot cues (including Shift+1 to 8, a full set of eight, and a refused
ninth), adds and deletes memory cues, checks marker pixels on the big and compact players, runs the CUE button
sequence, reloads and reselects. 0 console errors.
