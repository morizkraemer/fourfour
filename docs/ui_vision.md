# UI Vision

Design decisions for the production app UI. This is the *why* and the *how it should feel*. Companion to `ui_components.md` (the mockup checklist) and `ui-architecture.md` (technical/framework decisions).

---

## Guiding principles

1. **Not Rekordbox.** The reference is what to avoid: cluttered, modal, tab-heavy, every feature surfaced equally.
2. **One workspace, contextual surfaces.** No mode tabs. The right panel and bottom player adapt to what you're doing; the layout doesn't fragment into separate "screens."
3. **Discoverable primary paths.** Anything important must be visible. Right-click and drag are *shortcuts for power users*, never the only way to reach a feature.
4. **Everything hideable.** Defaults are opinionated. Visibility is not. Every sidebar section, panel, and chrome element has a hide toggle.
5. **Browse is the default state.** Most time is spent browsing/triaging, not curating. The default UI optimizes for that.

---

## Workflow stages

The app supports five real activities. Each maps to a UI shape, not a separate screen.

| Stage | What the user does | UI shape |
|---|---|---|
| Acquire | Drop new files into the library | Drop overlay, background analysis |
| Triage | Listen, decide, tag, comment | List + detail pane + numbered tag shortcuts |
| Organize | Bucket into playlists | Drag to playlist OR digit key tag |
| Curate | Build a set from library + playlists | Two-panel: source on left, target on right |
| Sync | Push to USB | Same as Curate, with USB target + sync chrome |
| Live prep | Per-track edits (cues, beat grid) | Expanded bottom player |

The transitions between these are **right-panel and player-region morphs**, not navigation. You don't change pages.

---

## Layout

```
┌──────────────────────────────────┐
│ sidebar │ list      │ right pane │   ← main area
│         │           │            │
├──────────────────────────────────┤
│ player (full window width)       │   ← compact or expanded
├──────────────────────────────────┤
│ status bar                       │
└──────────────────────────────────┘
```

**Top-level grid:** rows only (`main-area / player / status-bar`). The player and status bar span the full window width including over the sidebar. All column-splitting happens inside `main-area`.

**Main area:** three columns — sidebar (nav), list panel (active list), right panel (morphing).

---

## The morphing right panel

The right panel is the same DOM region in three modes:

| Mode | Contents | When |
|---|---|---|
| **Detail** | Selected track: artwork, full metadata, waveform preview, comments, cues, file path | Default — browse, triage, organize |
| **Target** | Pinned playlist or USB as a drop target, full track list | Curate or Sync |
| **Hidden** | — | Manual focus mode |

The header always labels the mode and provides a switcher / close button. The morphing is visible and controllable, not magic.

**Detail pane is the resting state.** The user does not need to "pin" anything to make the app useful. Detail is what you get for free.

---

## Entering Curate / Sync mode (target mode)

Right-click is not the primary path. The discoverable trigger is a **labeled button in the panel header** when viewing a playlist or USB:

- Viewing a playlist → header has a primary action button: "Curate →" or "Add tracks"
- Viewing a USB → header has "Load tracks" or "Sync"
- Clicking pushes the current view to the right panel as the target; the left panel becomes a navigable source

Power-user shortcuts (always work, never required):
- Drag a sidebar row to the right edge of the workspace
- Right-click → "Open as target"

Exit: close X on the right panel collapses it back to detail mode.

---

## Sidebar

The sidebar is **navigation only**, never a drop target for primary actions. Drag-and-drop is always panel → panel.

Structure (top to bottom, every section collapsible/hideable):

```
Tags ▾                 ← numbered favorites, our software's tag system
  1 ⚡ Peak       24
  2 🌊 Deep      18
  + press a digit

Library
  All Tracks
  Recently Added       ← smart filter: last 30 days
  Unfiled              ← smart filter: not in any playlist

Playlists           [+]
  Saturday
  Sunday Closing
  …

USB Devices
  ● SanDisk 64GB       ← mounted (green dot)
  ○ Tour Stick         ← unmounted (gray dot, persists)
```

**Drilldown for folders.** Sidebar uses sliding child pages (already in the mockup), not a treeview. One level visible at a time keeps it clean. Cross-folder operations happen via the *panel*, not the sidebar — open the source folder in the left panel, target is already pinned right.

**No second sidebar.** Two-sidebar layouts lead toward Rekordbox-mess. The left panel itself is navigable in target mode (breadcrumb / source picker in its header).

---

## USBs as persistent playlists

USBs live in the sidebar permanently — mounted or not. When ejected, you can still queue changes; they auto-sync on next mount.

Behaviorally identical to playlists. The only differences:
- Visual indicator for mounted/unmounted state
- Sync chrome (status, eject, wipe) when used as target
- A conflict-resolution flow if the USB's actual contents have drifted from your queued state

This collapses "curate" and "sync" into the same UI shape — the right panel just hosts a different target type.

---

## Tag system (numbered favorites)

Inspired by Ableton's favorites. **Not the same as CDJ color tags.** These are two independent fields.

- 9 numbered slots, user-named (default: empty)
- Press a digit while track(s) selected → tag with that slot. Press again → untag.
- Each slot is a smart playlist showing tagged tracks
- Multi-select + digit tags the whole selection
- Drag from a slot into a real playlist whenever you want to "promote" the buffer

**Layout:** vertical list at the top of the sidebar in a `Tags` section. Compact rows show digit + name + count. Empty slots are hidden until used (one "press a digit" hint row remains as guide). Section collapsible.

**Scope:** global and persistent. A tag is permanent metadata, not a session scratchpad. Right-click → "Clear slot" if you want to wipe one.

This dissolves the "Inbox / triage mode" idea entirely. The flow is: import → click Recently Added → press digits while listening. Tracks naturally end up tagged. No mode toggle, no special UI.

---

## Color tags vs number tags

Two separate features. Both are per-track metadata; neither lives in the sidebar.

| | Color tags | Number tags |
|---|---|---|
| Origin | CDJ hardware standard | Our software |
| Count | 8 fixed colors + none | 9 named slots |
| User renames | No | Yes |
| Syncs to USB | Yes (CDJ reads it) | No |
| Per-track | One color | Multiple slots possible |
| UI surface | Track row column | Track row column + sidebar shortcut |

Color tags appear as a small colored square in a tracklist column. They sit alongside other per-track metadata: comments, star ratings, BPM, key. None of these earn sidebar real estate.

---

## Search

Two distinct surfaces:

- **⌘F — inline filter.** Narrows the current list in place. Lives in the panel header. Standard, expected.
- **⌘K — command palette.** Raycast-style overlay. Searches tracks + playlists + USBs + actions ("Sync to SanDisk", "Add selected to Saturday"). Result behaviors: track → highlights in current list + opens detail; playlist → navigates sidebar; action → executes immediately.

Start v1 with track + playlist search in the palette. Action vocabulary expands later.

---

## Bottom player and track editor

The player spans the full window width. Two states, toggled with a shortcut (Ableton-style shift-tab):

- **Compact** — small bar with mini waveform, title, artist, play. Default.
- **Expanded** — grows upward to ~30–40% of viewport. Full waveform, beat grid, cue points, BPM/key fine-tune. The list and right panel above compress vertically. Both upper surfaces stay live and reflect the same selected track.

Track editing happens here, not in a modal or separate page. Avoids context loss.

---

## Track row metadata

Per-track fields that show as columns (toggleable show/hide via right-click on header):

- # (index)
- Title, Artist, Album, Label, Remixer
- BPM, Key, Time
- Genre, Year
- Color tag (colored swatch)
- Number tags (digit badges, multiple)
- Star rating
- Comment indicator
- Date added

The default visible set is conservative (Title / Artist / Label / BPM / Key / Time). Wider window or expanded mode reveals more. User can reorder and customize.

---

## Discoverability rules

- Primary actions: visible labeled button or persistent UI element
- Secondary actions: hover affordance (e.g. row hover arrow)
- Power-user: keyboard shortcuts, drag, right-click

Anything that lives only in right-click is by definition not the primary path. If we catch ourselves designing a critical action that only appears in a context menu, redesign.

---

## Responsive behavior

Desktop app, ≥1024×640 floor. Below threshold widths, surfaces auto-collapse:

- Below ~1100px: right panel auto-collapses
- Below ~900px: sidebar collapses to icon-only
- Below ~800px: undefined; user is on the wrong device

Don't optimize for phone-sized viewports.

---

## Things explicitly out of scope (for now)

- Smart playlists with rule editors (beyond Recently Added / Unfiled)
- Tag-and-playlist hybrid systems
- Light theme
- Mobile / touch
- Multi-window / detached panels
- Plugin / extension system

Revisit when the core flow is solid.
