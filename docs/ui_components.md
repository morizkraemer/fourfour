# UI Components & Mockup Checklist

What needs to be designed in Figma. Companion to `ui_vision.md` (the *why*).

Build order: foundations → components → core screens → overlays → edge states → settings.

---

## 1. Foundations

Lock these as Figma variables / styles before anything else.

- [ ] **Color tokens** — already defined in mockup CSS. Port to Figma vars:
  - `--bg-base` `#1e1e1e`, `--bg-elevated` `#242424`, `--bg-surface` `#2a2a2a`
  - `--selection` `rgba(0,99,225,0.32)`, `--selection-text` `#fff`
  - `--text-primary` `#e0e0e0`, `--text-secondary` `#999`, `--text-tertiary` `#5a5a5a`, `--text-disabled` `#404040`
  - `--border` `#333`, `--border-subtle` `#2a2a2a`
- [ ] **Type scale** — 13 / 12.5 / 12 / 11.5 / 11 / 10 px; weights 400 / 500 / 600; mono variant for tabular numbers
- [ ] **Spacing scale** — 2 / 4 / 6 / 8 / 12 / 16 / 24
- [ ] **Radii** — 3 / 4 / 6
- [ ] **Elevation** — menu shadow, dialog shadow
- [ ] **Iconography set** — sidebar icons (clock, folder, list, USB), panel actions (play, eject, sync, +, ×, search), sort indicators, color swatches, digit badges, status dots, drag handles

---

## 2. Components

Reusable building blocks. Design every state.

### Sidebar
- [ ] Sidebar row — default / hover / active / drag-over
- [ ] Sidebar row variants — leaf / folder (with chevron) / USB (with status dot) / tag (with digit badge + count)
- [ ] Sidebar section header — with optional `+` button and collapse chevron
- [ ] Sidebar back button (drilldown return)
- [ ] Sidebar drilldown transition states — root pushed, child visible
- [ ] Sidebar collapsed / icon-only mode

### Track list
- [ ] Track row — default / hover / selected / multi-selected / analyzing / drag-over / drag-source
- [ ] Track row metadata badges — color tag swatch, digit tag badge, star rating, comment indicator
- [ ] Track list column header — default / hovered / sorted (asc/desc) / dragged
- [ ] Empty row / empty-state placeholder

### Panels
- [ ] Panel header — title + subtitle + actions
- [ ] Panel header variants — library / playlist / playlist-as-target / USB-as-target
- [ ] Target chip / breadcrumb in header
- [ ] Panel actions area — primary CTA (Curate / Sync), secondary (Import / Analyze), close X

### Buttons
- [ ] Default / primary / destructive / icon-only / disabled
- [ ] Button group (segmented control if needed for player toggles)

### Overlays / popovers
- [ ] Dropdown menu
- [ ] Context menu
- [ ] Tooltip
- [ ] Dialog — text input variant
- [ ] Dialog — confirm variant (with destructive style)

### Player
- [ ] Compact waveform bar (bass/mid/high frequency-colored bars)
- [ ] Expanded waveform with beat grid
- [ ] Cue point marker (default / hover / dragging / labeled)
- [ ] Playhead line
- [ ] Play/pause button
- [ ] Time display
- [ ] Player mode toggle (color / mono / peaks)

### Status / feedback
- [ ] Status bar segments — spinner, message, progress, count
- [ ] Inline progress bar
- [ ] Drop overlay — drag-import variant
- [ ] Drop overlay — drag-to-panel variant
- [ ] Drag ghost — track row(s)

### Tags
- [ ] Tag row in sidebar (digit badge + name + count + hover state)
- [ ] Empty tag hint row
- [ ] Digit badge inline on track row

---

## 3. Core screens

Compose components into full-window states. Each is a Figma frame at default desktop size (~1440×900).

- [ ] **Browse — library + detail pane** (default, single track selected)
- [ ] **Browse — library + detail pane, multi-select** (count + bulk actions in detail pane)
- [ ] **Browse — playlist + detail pane**
- [ ] **Browse — viewing a tag slot** (e.g. "Peak Time")
- [ ] **Browse — Recently Added**
- [ ] **Browse — Unfiled**
- [ ] **Curate — library source + playlist target** (two-panel, idle)
- [ ] **Curate — mid drag-and-drop** (drop indicator visible)
- [ ] **Curate — playlist source + playlist target** (cross-playlist copy)
- [ ] **Sync — library source + USB target, mounted**
- [ ] **Sync — USB target, ejected** (queued changes indicator, dimmed actions)
- [ ] **Right panel hidden** (full-width list, focus mode)
- [ ] **Sidebar collapsed / icon-only** (small-window mode)

---

## 4. Player states

- [ ] Compact, track loaded, paused
- [ ] Compact, track loaded, playing (playhead position)
- [ ] Expanded — full editor (waveform, beat grid, cue strip, BPM/key edit controls)
- [ ] Expanded — cue point being dragged
- [ ] Expanded — beat grid being adjusted
- [ ] No track loaded

---

## 5. Detail pane content

Open question — needs design before mocking. Capture decisions in a short section here once chosen.

- [ ] Detail pane — single track selected (artwork, metadata, waveform thumb, comments, cues list, file path)
- [ ] Detail pane — multi-select (count, common metadata, bulk actions)
- [ ] Detail pane — no selection (placeholder)
- [ ] Detail pane — track has unresolved analysis / missing file

---

## 6. Overlays

- [ ] **⌘K command palette** — closed (just shortcut hint somewhere?)
- [ ] **⌘K command palette** — open, empty
- [ ] **⌘K command palette** — typing, results (track / playlist / action)
- [ ] **⌘F inline filter** — active in panel header, with results count
- [ ] **Right-click context menu — track row** (add to playlist, tag, color, edit, reveal in finder, delete)
- [ ] **Right-click context menu — playlist row** (rename, open as target, duplicate, delete)
- [ ] **Right-click context menu — column header** (show/hide columns, reset order)
- [ ] **Right-click context menu — tag row** (rename, clear all, color)
- [ ] **Drag ghost overlay** — single track / multiple tracks

---

## 7. Empty / loading / error states

- [ ] First launch — no library imported (onboarding)
- [ ] Library empty — after wipe / fresh install
- [ ] Playlist empty
- [ ] Tag slot empty (within Recently Added etc.)
- [ ] No tracks selected — detail pane placeholder
- [ ] No USBs connected
- [ ] Analysis in progress — status bar + per-row analyzing state
- [ ] Import in progress — drop confirmed, files copying
- [ ] Sync in progress — USB target with progress overlay
- [ ] **USB drift / conflict resolution** — physical USB contents diverged from queued state. Diff view (added / removed / changed). *Needs design before mocking.*
- [ ] File missing / track unplayable
- [ ] Analysis failed for a track

---

## 8. Settings / preferences

Open a separate window or full-screen page. Sections:

- [ ] **General** — library path, default theme, default sort, behavior toggles
- [ ] **View** — which sidebar sections visible, which columns visible by default, density
- [ ] **Analyzer** — BPM range, key notation (1A/Camelot vs sharp/flat), threading
- [ ] **Keyboard shortcuts** — list with rebinding
- [ ] **Storage / library health** — duplicates, missing files, orphan ANLZ files

---

## 9. Track editor (expanded player) — needs design

This is its own design session. Don't start mocking until the controls are decided.

Things to figure out:
- Cue point management — list view? inline-on-waveform? both?
- Beat grid editing controls — nudge, half/double, set first beat
- BPM / key fine-tune controls
- Memory cues vs hot cues — surface both?
- Saving / discard flow
- How it interacts with the right panel (both visible? does target mode collapse the editor?)

---

## Build order

1. Foundations (1) — half a day
2. Components (2) — most of the work; do every state
3. Core screens (3) using components — fast once components are done
4. Player states (4)
5. Overlays (6)
6. Empty / error states (7)
7. Settings (8) once everything else is stable
8. Detail pane (5) and Track editor (9) — design sessions first, then mock

Defer until a v2: USB drift conflict resolution, smart playlist editor, light theme.
