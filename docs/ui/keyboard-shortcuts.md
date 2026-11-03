# Keyboard shortcuts and command palette

Status: current. Code: `app/src/shortcuts/`.

## One registry

`app/src/shortcuts/app-actions.ts` is the single list of actions: id, label, group, default binding, `when` (availability) and `run`. Everything reads it:

- `App.svelte` has one `keydown` listener that calls `shortcuts.dispatch`.
- The command palette (`Mod+K`) lists the actions that are currently available, with their live bindings on the right.
- Preferences → Keyboard shortcuts lists every action by group and lets you rebind it.

A new shortcut is a new entry in that list. Do not add `keydown` handlers for global keys elsewhere.

## Bindings

`Mod` is Cmd on macOS and Ctrl elsewhere. A binding is stored as text such as `Mod+Shift+N`, `Tab`, `Space`.

Overrides are saved per user in `localStorage` under `fourfour.shortcuts.v1` (only the changed ones). Defaults can change without migration; unknown ids in storage are ignored.

Rebinding: click the binding, press the keys. Esc cancels, Backspace clears. If the keys belong to another action the row shows a message and offers Replace (the other action becomes unassigned) or Cancel. Reset restores one default, Reset all restores all.

## Defaults

| Action | Binding |
|---|---|
| Command palette | Mod+K |
| Open settings | Mod+, |
| Close settings, clear search, collapse player | Esc |
| Play / pause | Space |
| Search tracks | Mod+F |
| New playlist | Mod+Shift+N |
| Import folder | Mod+O |
| Toggle big / small player | Tab |
| Toggle organize drawer (right sidebar) | Shift+Tab |
| Focus next / previous region | F6 / Shift+F6 |
| Toggle favorite 1 to 9 on selection | 1 to 9 |
| Rate selection 1 to 5 stars, 0 clears (focus in the track list, not in a stick view) | Mod+1 to Mod+5, Mod+0 |
| Analyze selection, Sync stick, Toggle inspector, Show keyboard shortcuts | unassigned |

## Keys inside a focused control

These work only while that control itself has focus, so they never change the global keys.

| Control | Keys |
|---|---|
| Waveform (compact and expanded player, `role="slider"`, "Playback position") | Left / Right: one beat (one second without a beat grid). Shift+Left / Shift+Right: four bars (ten seconds without a grid). Home / End: track start / end. Value text is `m:ss of m:ss`. |
| Track list in a playlist (no sort active, no search filter) | Alt+Up / Alt+Down: move the selected tracks one row, saved like a drag reorder. The move is announced in the live region ("Moved X down, position 2 of 3", "Already at the top"). |

Rating uses Mod+digit because the plain digits are already the favorite toggles (1 to 9), and Shift+digit and Alt+Shift+digit are the hot cues. Mod+1 to 5 is also what Rekordbox and most players use for stars. The rating keys are the same registry actions as everything else, so they can be rebound in Preferences; they only fire when the track list has focus and not in an input.

The live region is one visually hidden `role="status"` element in `App.svelte`, driven by `announce()` in `stores/announce.svelte.ts`.

## Typing guard and accessibility

While a text input, textarea, contenteditable, dialog or Preferences has focus, only bindings with `Mod`, Esc or a function key fire. Plain keys, Space, digits, Tab and Shift+Tab keep their normal meaning there. An action can opt out with `inInput: false` (Import does).

Tab and Shift+Tab are no longer focus movement elsewhere. F6 and Shift+F6 move focus between the regions (sidebar, search, tracks, organize drawer, player; `data-focus-region` in `App.svelte`). Tab still works normally when no track is loaded, because the player toggle needs a loaded track.

## Palette

`Mod+K` opens a centred overlay. Type to fuzzy-match actions, playlists/devices ("Go to") and tracks (title and artist). Arrows or Tab/Shift+Tab move, Enter runs, Esc closes. Matching is in `shortcuts/fuzzy.ts`, grouping in `shortcuts/palette-model.ts`, the overlay is the design-system `CommandPalette`.
