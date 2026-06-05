# Organize Right-Sidebar

**Date:** 2026-06-05
**Status:** implemented (all 8 slices on `feat/organize-sidebar`)

## Implementation notes (2026-06-05)

All slices s1–s8 implemented and committed; `npm run build` green throughout.

- **s1** replaced `splitPanel`/dual-`ListPanel` with `ui.organize` + `OrganizePanel.svelte` (rail/expanded, magnetic snap, localStorage geometry). Removed all split machinery from ui/dnd/table-columns/Sidebar/context-menus/ListPanel/TrackTable.
- **s2** `OrganizePicker.svelte` (New playlist · search ≤4 / 4 recents · USB) + `dockOrganizeTarget` + renders docked target via `TrackTable`.
- **s3** `'organize'` LayoutKind (index, cover, title, album, bpm, key), independent + persisted; `TrackTable` gained `layoutKindOverride`.
- **s4** inspector hidden while panel expanded — pure derived condition in `App.svelte` (no new state).
- **s5** `SidebarRow` hover "open in side panel" (favorites/playlists/USB only) + sidebar ＋ auto-docks new playlist.
- **s6** drop left→right: playlist adds at drop index (`moveTracksToPlaylist insertIndex`); USB triggers `exportTracksToVolume`.
- **s7** persist + reconcile docked target across restarts (localStorage; validates against live playlists/volumes after library load).
- **s8** filled-collapsed rail shows up to 4 covers + track count.

**Known gaps / follow-ups:**
- USB drop export uses the backend `write_usb`, which is a **holistic full-library sync**, not per-track — dragged ids are logged for future selective export. Progress/error feedback still TODO (already flagged out-of-scope).
- `ListPanel.svelte` is now unused but retained (retire in a later cleanup, salvaging header bits if any remain useful).
- An unrelated scrub-pause player/waveform change surfaced during the work is held in `git stash` (not part of this feature).

## Problem

Side-by-side curation already exists (`ui.splitPanel` + `ListPanel` + cross-panel dnd in `dnd.svelte.ts`), but it's the Rekordbox trap: it's reachable only via a hover "split" icon nobody discovers, and opening it **couples both panes and replaces the entire content region** — destroying your library browse position and the inspector. So users fall back to dragging tracks onto sidebar playlist rows (the one always-visible target), and the powerful organize flow goes unused.

We want organizing/preparing sets to be the *intuitive first choice*, without making side-by-side the default and without hiding it behind a button. Browsing + listening stays the resting default.

## Solution

Replace `splitPanel` with a **persistent right-sidebar organize panel** that is independent of left navigation:

- **Permanent right-edge rail** — always visible (the discoverability bet). Empty = "pull me out" handle; filled-collapsed = covers + count glance.
- **Width spectrum** via inner-border drag handle: **rail → partial → fullscreen**. Magnetic snap only at the two extremes; free resize in between. The browse library is always the base underneath, returned to on collapse.
- **Two independent panes:** left/main is driven by sidebar navigation (single-click opens here — library views, playlists, USB); right is a **sticky destination** (playlists, favorites, USB only — no library views).
- **Entry:** new playlists are the *only* thing that auto-docks right (expanded, inline-name). Everything else opens left; right-docking anything existing is explicit — via the **pull-out picker** (＋New playlist · search playlists showing 4 results, empty = 4 recents · USB devices) or the hover "open in side panel" icon (overwrites).
- **Right content:** full `TrackTable` with its *own* persistent column layout (defaults: number, cover, title, album, bpm, key), independent from the left table.
- **Drop left→right:** playlist = add reference; USB = export (the point).
- **Inspector** is the default occupant of the right-region: coexists with the rail sliver, gets borrowed (closed) when the panel expands, restored on collapse. No inspector while organizing.
- **Persistence:** docked target + panel geometry restored across restarts. No ephemeral scratchpad — new playlists *are* the scratchpads. **Switch** = hover-open/picker (overwrite); **collapse** = park to rail; **close** = clear target.

## Out of scope

- **Edit mode** — a later, separate fullscreen *mode* in the right panel (not a tab, possibly its own design). Designed separately.
- **USB write pipeline internals** — drop-onto-USB triggers the existing `writer::filesystem::write_usb` path; we only wire the trigger + feedback, not the format code.
- **Tabs / stacked targets** in the right panel — single target only; a future power feature.
- **Inspector peek while organizing** — deferred; for now no inspector when the panel is expanded.
- **Rewriting cross-panel dnd plumbing** — reuse the existing `dnd.svelte.ts` machinery.

## Slices

### s1: Right-sidebar shell — rail, pull-out, resize, collapse, close
- **outcome:** A permanent right-edge rail is always visible; you can pull it out to an (empty) panel, drag-resize it (magnetic at rail/fullscreen, free between), collapse back to the rail, and close it. Replaces the `splitPanel` layout branch.
- **depends_on:** none
- **likely_files:** `app/src/App.svelte`, `app/src/stores/ui.svelte.ts`, new `app/src/modules/OrganizePanel.svelte`, new rail/handle component(s), `prototyping/viewport/design-system/` (new primitives if needed)
- **acceptance:**
  - [ ] Rail sliver visible on the right edge in the default browse state (empty "pull me out").
  - [ ] Pull-out expands to a panel; inner-border drag resizes freely and snaps to rail (near inner edge) and fullscreen (near outer edge).
  - [ ] Collapse button returns to rail; close button clears the panel; both states distinct.
  - [ ] Old `ui.splitPanel` / two-`ListPanel` layout no longer used.

### s2: Destination picker + dock + render target
- **outcome:** Pulling out the empty rail shows the picker (＋New playlist · search playlists with 4 results, empty = 4 recents · USB devices); picking one docks it on the right and renders its tracks (via `TrackTable`).
- **depends_on:** s1
- **likely_files:** `app/src/modules/OrganizePanel.svelte`, new `OrganizePicker.svelte`, `app/src/stores/library.svelte.ts` (recents, playlist search), `app/src/stores/ui.svelte.ts` (organize target state), `app/src/modules/TrackTable.svelte`
- **acceptance:**
  - [ ] Empty pulled-out panel shows New playlist, a playlist search with ≤4 results (4 recents when empty), and mounted USB devices.
  - [ ] Picking a playlist or USB docks it and shows its tracks on the right.
  - [ ] "New playlist" creates a real playlist immediately, docked + expanded, inline-named.
  - [ ] Read-only library views (All Tracks / Recently Added) are not offered as destinations.

### s3: Independent persistent column layout for the right table
- **outcome:** The right table has its own column config (resize/order) persisted separately from the left, defaulting to number, cover, title, album, bpm, key.
- **depends_on:** s2
- **likely_files:** `app/src/stores/table-columns.svelte.ts` (split into left/right contexts), `app/src/modules/TrackTable.svelte`, `app/src/modules/OrganizePanel.svelte`
- **acceptance:**
  - [ ] Right table renders the 6 default columns; left table keeps its own layout.
  - [ ] Resizing/reordering columns on one side does not affect the other.
  - [ ] Each side's column layout persists across reloads.

### s4: Inspector state machine
- **outcome:** Inspector coexists with the rail sliver; expanding the organize panel closes it; collapsing restores it (if a track is selected). No inspector while the panel is expanded.
- **depends_on:** s1
- **likely_files:** `app/src/App.svelte`, `app/src/modules/Detail.svelte`, `app/src/stores/ui.svelte.ts`
- **acceptance:**
  - [ ] Browse + selection + collapsed rail → `library | inspector | rail` all visible.
  - [ ] Expanding the panel hides the inspector; collapsing brings it back for the current selection.
  - [ ] Selecting a track while expanded does not open the inspector.

### s5: Hover "open in side panel" (destinations-only) + new-playlist-auto-right
- **outcome:** The sidebar hover "open in side panel" icon docks that source on the right (overwriting the current target), shown only on destinations (playlists/favorites/USB); creating a new playlist anywhere auto-docks it right.
- **depends_on:** s2
- **likely_files:** `app/src/modules/Sidebar.svelte`, `app/src/stores/library.svelte.ts`, `app/src/stores/ui.svelte.ts`
- **acceptance:**
  - [ ] Hover-open appears only on playlists/favorites/USB rows, not on library views.
  - [ ] Hover-open overwrites the docked target without confirmation (targets are concrete/saved).
  - [ ] Sidebar ＋ new-playlist creates and auto-docks it expanded on the right.

### s6: Drop left→right — add / export
- **outcome:** Dragging tracks from the left into the docked target adds them (playlist) or exports them (USB), reusing the existing dnd machinery.
- **depends_on:** s2
- **likely_files:** `app/src/stores/dnd.svelte.ts`, `app/src/modules/OrganizePanel.svelte`, `app/src/stores/library.svelte.ts`, `app/src/services/tauri.svelte.ts` (USB export trigger)
- **acceptance:**
  - [ ] Drop onto a docked playlist adds the tracks (reference) at the drop index.
  - [ ] Drop onto a docked USB triggers export of those tracks to the device.
  - [ ] Drop is disabled for any non-writable target.

### s7: Persistence of docked target + geometry
- **outcome:** On relaunch, the right panel restores its docked target and geometry (width + collapsed/expanded).
- **depends_on:** s2
- **likely_files:** `app/src/stores/ui.svelte.ts`, `app/src/services/tauri.svelte.ts` (save/load state)
- **acceptance:**
  - [ ] Docked target id, panel width, and collapse state survive a restart.
  - [ ] A closed panel restores as empty rail (not re-docked).

### s8: Filled-collapsed covers glance
- **outcome:** When a target is docked but collapsed, the rail shows a covers + count glance instead of the empty "pull me out" handle.
- **depends_on:** s2
- **likely_files:** rail component, `app/src/modules/OrganizePanel.svelte`, `app/src/stores/library.svelte.ts`
- **acceptance:**
  - [ ] Empty rail shows the "pull me out" handle; docked-collapsed rail shows recent covers + track count.
  - [ ] Clicking the filled rail expands back to the last geometry.

## Dependency graph

```
s1 → s2, s4
s2 → s3, s5, s6, s7, s8
```

## Parallel batches

- **Batch 1** (independent): s1
- **Batch 2** (after s1): s2, s4
- **Batch 3** (after s2): s3, s5, s6, s7, s8

## Notes

- **Replaces, not adds:** `ui.splitPanel`, the dual-`ListPanel` branch in `App.svelte`, and `ListPanel.svelte` itself are superseded. `OrganizePanel` reuses `TrackTable` directly; salvage any still-useful header bits from `ListPanel` then retire it.
- **Project convention:** new visual primitives (rail handle, picker) should be built/verified in `prototyping/viewport/design-system/` first per the ui-prototyping workflow, then consumed via `$ds` in the app modules.
- **dnd reuse:** `dnd.svelte.ts` already has `updateTrackDropTarget`, `crossPanelDrag`, and drop-index hit-testing. s6 should extend the existing drop-target resolution to the docked panel rather than rewrite it.
- **USB export feedback** (progress/errors for s6 drops) is worth a follow-up — heavy operation, currently fire-and-forget would feel broken. Flag for a later slice if not handled inline.
- **Open:** exact magnetic-snap thresholds (px from each edge) and default partial width — tune during s1.
- Edit mode is the next design grill (separate doc); it will inhabit this panel at fullscreen, so keep the panel a general "focus surface," not playlist-specific.
```