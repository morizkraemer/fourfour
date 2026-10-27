# Design round 3: inspector, calmer (F22)

Date: 2026-10-27. Branch `wi-inspector3`, from `q41-feedback`. Mockup boards only; no app code changed.
Run `cd prototyping && npm run mockups`, open `http://127.0.0.1:5230/#inspector-3` (workflow in [mockups.md](mockups.md)).
Round 2 ([design-round-wm-2.md](design-round-wm-2.md)) stays on the canvas as page `inspector-2`.

## Problem

Merlin on round 2: too overwhelming and crazy, no clear visual hierarchy, and no ring layout for keys.
Round 3 keeps the F22 goals (every field editable, hand-set values visible) and drops the busyness:

- One primary block (artwork, title, artist), then BPM, key and length, then quiet metadata.
- Whitespace and type scale do the work; almost no borders, boxes or extra colours.
- Edit on click; the resting view is plain text. BPM and key are the existing `ScrubValue`, key in its Camelot table colour. No wheel.
- Hand-set values get one small padlock (`LockMark`), not a badge per field.
- Cues are a short list with readable names. Placement and width unchanged (340 px, right of the list, hidden while the right sidebar is open).

Reused as they are: `GhostField`, `LockMark`, `ScrubValue`, vanilla stars, the strip waveform. Nothing new in the design system.

## Options

| | Idea | Good | Cost |
|---|---|---|---|
| A. Document (`inspector-3/option-a`) | One quiet column in order of importance: cover and title, three figures, one lock line, plain label and value rows, short cue list, file path | Everything visible and one click from editable; smallest build | Longest of the three; no picture of the track |
| B. Hero and folds (`option-b`) | Cover as a hero with the title on it, one strip of BPM, key, length and a padlock; folds: Cues open, Details and File closed with a summary line | Calmest at rest; fits any height; summaries answer most glances | Tags one click further away; hero uses 168 px of height |
| C. Waveform spine (`option-c`) | Title and figures, then a waveform with cue flags and the cue list under it; tags shrink to one line plus album, stars, colour, comment | Cues seen where they are; short panel | Tags in a line are harder to hit; needs a waveform strip in the panel |

Self-critique ("would Merlin call this overwhelming?"): the first drafts used boxed tiles, a pill-shaped lock label and a 5-chip colour row; all removed. Colour is now a single dot, rating plain stars, lock one icon. C's tag line was cut off at 340 px and was split into two lines. A is the busiest simply because everything is visible; B is the quietest.

## Recommendation

B. The hierarchy is the layout (image and title, figures, folds), the resting view is a handful of words, and cues never fall off the screen. If tags are edited often, start with Details open, which is A.

## Open questions

1. Details open or closed by default?
2. One padlock for BPM and grid: enough, or show which one is hand-set (tooltip only)?
3. Cue times typed is fine for now, or is a waveform (C) a must?
4. Hero at 168 px: OK, or smaller?

## Built: option A, with Merlin's changes (branch `wn-inspector`)

Order top to bottom: cover with title and artist, three dark cards (BPM, key, length), one hand-set line, the waveform with cue markers, a compact cue list, plain label and value rows, the file path.

- BPM and key are the existing `ScrubValue` (key in its Camelot colour); length is read-only.
- The waveform is the existing `Waveform` strip (3-band look, analysing animation). Click seeks; a track that is not in the player is loaded first. Hot cues are lettered markers, memory cues thin marks; a marker click jumps there.
- The strip is fed the same series the player's overview draws (the analysis payload's overview, else its detail columns), not the 128-column row preview: that preview is cut from whichever series the analysis stored and carries no scale, so detail-sourced previews drew as a solid blue block.
- Right-click on a cue row or marker opens the app menu: Jump to, Rename, Colour (hot only), Convert to memory/hot, Delete. Hot cues show their pad letter in colour; memory cues a hollow neutral mark, in the list, the legend and on the strip.
- Editing a figure in place keeps the resting font and stays inside its card. The webview's own menu (Reload, Inspect Element) is suppressed in production builds outside text fields.
- Width is 340 px (was 278, min now 300): three cards with BPM at two decimals do not fit in 278. Still hidden while the right sidebar is open.
- Title, artist, album and genre show as plain text: `DetailPane` takes `onFieldChange` and turns the rows into `GhostField`s, but no Tauri command writes tags yet (`Library::update_track` exists in `pioneer-library` without a command). Label, year, comment, rating and colour have no data in the app yet, so no rows.
