# Design round 2: inspector (F22) and settings (F21)

Date: 2026-10-26. Branch `wm2-mockups`, from `q41-feedback`. Mockup boards only; no app code was changed.
Run `cd prototyping && npm run mockups`, open `http://127.0.0.1:5230/#inspector-2` and `#settings-2` (workflow in [mockups.md](mockups.md)).
Round 1 is in [design-round-wm-1.md](design-round-wm-1.md). Its inspector and settings boards stay on the canvas as pages `inspector` and `settings`; round 2 lives in `inspector-2` and `settings-2`.

Each page has a "now" board (today, built from the real design-system components), the option boards and a Decisions board.
The judgement is intuitiveness and speed of use, not Rekordbox familiarity.

## New design-system primitives

Needed by every inspector option and by the lock state; added to `prototyping/viewport/design-system` and exported from `svelte.js`.

| Primitive | What it is |
|---|---|
| `GhostField` | A text value that looks like plain text. Hover shows a pencil, click makes it an input, Enter or blur commits, Esc reverts. Supports multiline, mono, `mixed` (multi-select) and an autocomplete list. |
| `LockMark` | Padlock for "hand-set, kept across re-analysis". Closed = yours, open (dashed) = analysis may replace it. Icon alone or with a label ("Yours"). One click toggles. |
| icons | `lock`, `lock-open`, `pencil` added to the glyph set. |

Reused as they are: `ScrubValue` (BPM, key), `CueEditor`, vanilla `createStars`, `createColorPicker`, `createToggle`, `SegmentedControl`, `Dropdown`, `Waveform`, `SettingsPage`, the command palette classes.
Board-local sketches (not for the app): the zoomed cue lane (`W2Wave`), the Camelot wheel, the keyboard map, the tiles.

## 1. Inspector (F22)

Placement is fixed: right of the open track list, inside the main view. Edits write to the library only, never to file tags (a line on the Tags side says so).

### Problem

Today's inspector (278 px) is a read-only summary. Only BPM and key (ScrubValue) and the cue chips (rename, recolour, delete) can be changed. Title, artist, album, label, genre, year, rating, colour and comment cannot, and genre, year, rating, colour and comment are not even shown. Cue times cannot be edited, grid actions live in a context menu, and the 290 px cover pushes the data under the fold.

The lock is invisible. The library keeps four flags per track in `edits.json` (`bpm_locked`, `key_locked`, `grid_locked`, `cues_locked`); re-analysis leaves locked values alone and Rekordbox-imported grids and cues are locked too. The only trace in the UI is a right-click item "Let re-analysis replace BPM and key", which frees all four.

### What is inline, what is on focus (same in every option)

| Field | Edit |
|---|---|
| Title, artist, album, genre, label, year, comment | Inline `GhostField`, Enter commits, Esc reverts; genre and label autocomplete from the library |
| Rating | Click stars (1 to 5 on the keyboard when focused, 0 clears) |
| Colour | The 7 CDJ colour chips, one click |
| BPM, key | `ScrubValue` as today (drag, wheel, type, menu). Option B adds a Camelot wheel popover for the key |
| Grid | Never typed: Shift, ×2, ½, Fit to BPM, Re-detect, Set downbeat. "Fit" stays a separate explicit step |
| Cues | Name and time as fields, colour from the palette, delete, add at playhead; beat nudges; drag on a waveform in option C |
| Lock | `LockMark` on BPM, key, grid and cues. Hand-set shows closed with "Yours" and the analysed value next to it with "Use"; analysed shows quiet text (A, C) or an open padlock (B) |

Multi-select: fields show "Mixed" and write to every selected track.

### Options

| | Idea | Good | Cost |
|---|---|---|---|
| A. Form column (`inspector-2/option-a`) | One scrolling column of ghost fields in sections; cues as rows with name and time fields | Nothing hides behind a mode; smallest build; lock sits on the value it protects | Long: grid, cues and file fall under the fold on 900 px; cue times typed, no picture of the track |
| B. Tiles and focused editors (`option-b`, `option-b-cue`) | Compact card; six tiles open the editor that fits (key wheel, grid menu, cue editor plus time row); padlock on each tile | Fits 900 px without scrolling; each value gets the right editor | Two clicks per edit; popovers hide fields below; many small editors to keep consistent |
| C. Prep and Tags (`option-c`, `option-c-tags`) | A two-way switch. Prep: tempo, key, grid, cues around an overview plus a zoomed lane (16 beats) where cues are dragged and snap to beats. Tags: every text field, rating, colour, read-only file block | Cue and grid work happens on a waveform; the lock gets room (analysed value beside yours); tags never push cues off screen | Two modes; needs a zoomable waveform in a side panel (new work, overlaps the expanded player) |

Panel width in A and B is 340 px, in C 380 px (today 278, resizable up to 520).

### Recommendation

C. Fixing a cue or a grid is a picture problem; a typed time is slow and blind, a flag dragged on a zoomed lane and snapped to the beat is how a DJ thinks. Typing tags and checking a grid are different moods, so splitting them keeps rare fields from pushing cues off the screen. The Tags side is A's ghost fields, so that half is cheap.

Build order: Tags side first (GhostField and LockMark exist), then Prep with the zoomed lane. If the lane is too big for now, ship A's cue rows plus beat nudges and keep C as the target.

### Open questions for Merlin

1. Should a padlock be clickable without editing (lock a good analysis so re-analysis never touches it), or does a lock only come from your own edit?
2. Lock granularity: today it is four flags and "release" frees all four. Release one at a time in the UI?
3. Rekordbox-imported grids and cues are locked too. Show them as "Imported" (third origin) or just "Yours"?
4. Cue drag: snap to beats by default with Alt for free, or free by default?
5. Should Prep share its waveform with the expanded player, or stay a separate small view?
6. Panel width: OK to widen from 278 to 340 or 380, and does the list still feel big enough at 1280 px?
7. Multi-select: batch edit of tags only (Mixed), or also BPM and key offsets?

## 2. Settings (F21)

### Inventory (what exists today)

| Where | Setting | Notes |
|---|---|---|
| General | Library database path + Change | `library.db`, WAL and the ANLZ cache live there |
| General | Import from Rekordbox | Opens a dialog: master.db, USB stick or rekordbox.xml; skip or keep missing files |
| General | App version | Read-only |
| Audio | Playback engine | A dropdown with one entry (CoreAudio) |
| Audio | Output device | System default or a CoreAudio device; stops playback on change |
| Audio | Sample rate | 44.1 / 48 / 96 kHz |
| Audio | Buffer size | 64 / 128 / 256 / 512 / 1024 frames |
| Keyboard shortcuts | 42 actions in 6 groups | Click to record, Esc cancels, Backspace clears, Replace / Cancel on conflict, per-row Reset, Reset all |
| Debug | Library counts, IPC diagnostic, last error | Read-only |
| Debug | Reset library database | Destructive, confirm dialog |
| No UI yet | Analyze new tracks on import | `auto_analyze_new_tracks` in config.json, default on, commands `get/set_auto_analyze_new`. After an import a dialog "Analyze imported tracks?" asks every time |
| No UI yet | Analysis engine | `analysis_backend` rust or python (config key), falls back to Python without model weights |
| No UI yet | Analysis worker count | `FOURFOUR_ANALYSIS_THREADS` environment variable only |
| None | Library folders | The app imports on demand; there are no watched folders. The "library path" is the database location |

### Problem

Settings is a full-window takeover with one 680 px column hugging the left. Every row is a label plus a two-line description, so four settings fill a screen. The library, audio, shortcuts and a destructive reset have equal weight. There is no analysis section, so the three analysis settings cannot be reached. The shortcut page is a 42-row list with no search; hot cue A to H and favorites 1 to 9 take 25 of those rows. You cannot hear whether an audio change worked without closing settings and pressing play.

### Options

| | Idea | Good | Cost |
|---|---|---|---|
| A. Sheet by object (`settings-2/option-a`, `option-a-keys`) | Centred sheet over the library. List by object (Library, Analysis, Audio, Keyboard, Advanced), each with its live state. Short pages: status card, compact rows. Keyboard page: filter, group chips, two columns | First look explains itself; Analysis exists (import toggle, engine, workers, Pause and Cancel on the live queue); compact rows | Modal: cannot hear a change with the player in view; shortcuts still long |
| B. Settings in the palette (`option-b`, `option-b-keys`) | Settings are rows in ⌘K with their control on the right; searched together with commands and their shortcuts. A keyboard map replaces the shortcut list (layers Plain, ⇧, ⌘, ⌥⇧; click a key to see or change it) | Fastest for a known setting (⌘K, type, change); no new window; the map shows what is on each key and what is free | You must know a setting exists; no room for explanations or live feedback; the map is a new custom widget and depends on keyboard layout |
| C. Side panel (`option-c`) | The right slot becomes a Settings panel from the status-bar gear. Accordion with a summary per section, non-modal. Audio has a level meter and Test tone | Change the buffer or device and play, with the library still visible; collapsed sections show their state | 400 px: labels only; replaces the inspector while open; shortcuts need the map or a sheet |

The keyboard map is independent of the structure and can be combined with any option.

### Recommendation

A as the home, B as the fast path. Settings are opened rarely, so the first look should explain itself (a short page with a live status card). The palette already exists and already lists shortcuts, so rows with a control are cheap and make a known setting three keys. The keyboard map replaces the shortcut list later. Steal C's level meter and Test tone for A's Audio page.

Order: the Analysis page first (it exposes settings that have no UI), then palette rows, then the map. Debug moves under Advanced; Reset library sits at the bottom of Library as a danger zone.

### Open questions for Merlin

1. "Analyze new tracks on import" as a toggle (default on) with the "Analyze imported tracks?" dialog gone, or keep asking each time?
2. Expose the analysis engine (Built-in or Python) and the worker count, or keep them in config for now?
3. Library folders: do you want watched folders ("keep these folders in sync"), or is that out of scope?
4. Reset library: Library page (danger zone) or Advanced?
5. Keyboard map: worth building (ANSI, ISO and German layouts), or filter plus two columns is enough?
6. Sample rate and buffer: behind an "Advanced audio" fold, since most people never change them?

## Verification

All boards were loaded headless (Playwright) with no console errors and no overflow outside their artboards. Screenshots: `scratchpad/feedback/wm2/` of the session. The dev server for the check ran on port 5232 because 5230 was taken by another worktree's board server.
