# fourfour — Design System

LLM-readable component spec. Reference this when implementing UI in any framework.
Brand 11 (Geist Sans, achromatic). Visual reference: `mockup/brand-11.html`.

Companion to `ui_vision.md` (the *why*) and `ui_components.md` (the build checklist).

---

## How to use this file

Each component below is specified as:
- **id** — stable identifier
- **purpose** — one-sentence intent
- **anatomy** — element parts
- **states** — every visual state with the tokens that change
- **sizes** — when more than one
- **tokens** — list of design tokens consumed
- **rules** — invariants that must hold

When implementing, read the relevant component spec, use the tokens by name (do not hardcode values), and treat `rules` as MUST.

Convert tokens to whatever format your stack needs (CSS vars, Tailwind config, JS object). The names are normative.

---

## 1. Tokens

### 1.1 Color

```json
{
  "color": {
    "bg":          { "value": "#0a0a0a", "use": "app background, deepest layer" },
    "surface":     { "value": "#131313", "use": "panels (list, detail, settings)" },
    "elev":        { "value": "#1c1c1c", "use": "menus, dialogs, toasts, segmented control bg" },
    "elev-hi":     { "value": "#242424", "use": "selected segmented item, hovered default button, raised badge" },
    "text":        { "value": "#ededed", "use": "primary text" },
    "text-mid":    { "value": "#aaaaaa", "use": "secondary text, muted icons" },
    "muted":       { "value": "#6a6a6a", "use": "tertiary text (sub labels, meta)" },
    "faint":       { "value": "#4a4a4a", "use": "quaternary (placeholder, disabled, separators in text)" },
    "border":      { "value": "rgba(255,255,255,0.06)", "use": "default 1px borders between surfaces" },
    "border-hi":   { "value": "rgba(255,255,255,0.12)", "use": "interactive borders (inputs, kbds, dialogs)" },
    "hover":       { "value": "rgba(255,255,255,0.04)", "use": "row hover background" },
    "select":      { "value": "rgba(255,255,255,0.07)", "use": "selected row background" },
    "accent":      { "value": "#fafafa", "use": "primary action bg, playhead, focus ring tint" },
    "danger":      { "value": "#f87171", "use": "destructive text/border, error inline alert" },

    "track-color-1": { "value": "#ec4899", "use": "CDJ color tag — pink" },
    "track-color-2": { "value": "#f59e0b", "use": "CDJ color tag — orange" },
    "track-color-3": { "value": "#facc15", "use": "CDJ color tag — yellow" },
    "track-color-4": { "value": "#4ade80", "use": "CDJ color tag — green" },
    "track-color-5": { "value": "#38bdf8", "use": "CDJ color tag — blue" },
    "track-color-6": { "value": "#a78bfa", "use": "CDJ color tag — purple" },
    "track-color-7": { "value": "#94a3b8", "use": "CDJ color tag — grey" },

    "key-1":  { "value": "#f43f5e", "use": "Camelot key wheel — 1A/1B" },
    "key-2":  { "value": "#fb923c", "use": "Camelot key wheel — 2A/2B" },
    "key-3":  { "value": "#f59e0b", "use": "Camelot key wheel — 3A/3B" },
    "key-4":  { "value": "#eab308", "use": "Camelot key wheel — 4A/4B" },
    "key-5":  { "value": "#84cc16", "use": "Camelot key wheel — 5A/5B" },
    "key-6":  { "value": "#22c55e", "use": "Camelot key wheel — 6A/6B" },
    "key-7":  { "value": "#14b8a6", "use": "Camelot key wheel — 7A/7B" },
    "key-8":  { "value": "#06b6d4", "use": "Camelot key wheel — 8A/8B" },
    "key-9":  { "value": "#38bdf8", "use": "Camelot key wheel — 9A/9B" },
    "key-10": { "value": "#3b82f6", "use": "Camelot key wheel — 10A/10B" },
    "key-11": { "value": "#8b5cf6", "use": "Camelot key wheel — 11A/11B" },
    "key-12": { "value": "#ec4899", "use": "Camelot key wheel — 12A/12B" },

    "status-online":  { "value": "#4ade80", "use": "USB mounted dot" },
    "status-warn":    { "value": "#fbbf24", "use": "modified marker in conflict diff" },
    "status-offline": { "value": "#4a4a4a", "use": "USB ejected dot" }
  }
}
```

**Rules:**
- Chrome (sidebar, panels, buttons, inputs) is achromatic only. Never use accent color for chrome.
- The CDJ track colors appear only on track rows, color picker, and cue point pips.
- `danger` is the single allowed chromatic accent and only on irreversible actions.

### 1.2 Typography

```json
{
  "font": {
    "family":     { "value": "'Geist', system-ui, sans-serif" },
    "family-mono":{ "value": "'Geist Mono', ui-monospace, monospace", "use": "BPM, time, key, paths, section labels" }
  },
  "type": {
    "display":    { "size": "30px", "weight": 600, "tracking": "-0.025em", "line": 1.1,  "use": "first-launch wordmark, panel hero number" },
    "h1":         { "size": "18px", "weight": 500, "tracking": "-0.015em", "line": 1.3,  "use": "settings page title" },
    "h2":         { "size": "14px", "weight": 500, "tracking": "0",        "line": 1.3,  "use": "panel title, dialog title, detail pane title" },
    "body":       { "size": "13px", "weight": 400, "tracking": "0",        "line": 1.45, "use": "default body" },
    "row":        { "size": "12.5px","weight": 400, "tracking": "0",       "line": 1.4,  "use": "list rows, sidebar rows" },
    "compact":    { "size": "12px", "weight": 400, "tracking": "0",        "line": 1.4,  "use": "track-row cells, button label" },
    "caption":    { "size": "11.5px","weight": 400, "tracking": "0",       "line": 1.4,  "use": "subtitle, count, meta" },
    "small":      { "size": "11px", "weight": 400, "tracking": "0",        "line": 1.4,  "use": "kbd, status bar text" },
    "num":        { "size": "11px", "weight": 400, "family": "mono", "use": "tabular numbers in track rows" },
    "label":      { "size": "10px", "weight": 500, "family": "mono", "tracking": "0.06em", "case": "uppercase", "use": "section headings, dt labels" },
    "label-sm":   { "size": "9.5px","weight": 500, "family": "mono", "tracking": "0.06em", "case": "uppercase", "use": "column headers, sidebar section heads" }
  }
}
```

**Rules:**
- Mono is reserved for: tabular numbers, uppercase labels, file paths, kbd shortcuts.
- Never use mono for prose, titles, or button text.
- Tabular numbers must use `font-variant-numeric: tabular-nums`.

### 1.3 Spacing

```json
{
  "space": {
    "0": "0px",
    "1": "2px",
    "2": "4px",
    "3": "6px",
    "4": "8px",
    "5": "12px",
    "6": "14px",
    "7": "16px",
    "8": "24px",
    "9": "32px",
    "10": "48px",
    "11": "56px"
  }
}
```

Defaults: row vertical padding = 6 / horizontal = 12. Panel padding = 14. Section margin = 56.

### 1.4 Radii

```json
{
  "radius": {
    "xs": "2px",
    "sm": "3px",
    "md": "4px",
    "lg": "5px",
    "xl": "6px"
  }
}
```

Usage: pills/digits = xs · row hover/select = sm · buttons/inputs/lists = md · menus/dialogs = lg · panels/dialogs = xl.

### 1.5 Elevation (shadows)

```json
{
  "shadow": {
    "menu":   "0 6px 24px rgba(0,0,0,0.5)",
    "toast":  "0 8px 28px rgba(0,0,0,0.5)",
    "dialog": "0 12px 40px rgba(0,0,0,0.6)",
    "palette":"0 16px 60px rgba(0,0,0,0.7)"
  }
}
```

### 1.6 Motion

```json
{
  "motion": {
    "instant":  "60ms",
    "fast":     "80ms",
    "default":  "120ms",
    "slow":     "200ms",
    "easing":   "ease"
  }
}
```

Hover transitions = fast. Slide/expand = default. Loading bars = slow.

### 1.7 Iconography

- Stroke icons: 1.2px stroke at 12px viewbox, 1.4px stroke at 24px viewbox, `stroke-linecap: round`, `stroke-linejoin: round`.
- Filled icons: solid `currentColor` (inherit from parent).
- Icon sizes: `xs 9px` · `sm 11px` · `md 13px` · `lg 14px` · `xl 22px` (player cover) · `2xl 50%-of-art` (detail pane cover).
- Use `<symbol>` + `<use>` for repeated icons (note, star, etc).

---

## 2. Components

Each spec is the contract. States listed are exhaustive — if a state isn't here, it doesn't exist.

### 2.1 button

```json
{
  "id": "button",
  "purpose": "Trigger an action. Always labeled (icon-only is a separate variant).",
  "anatomy": ["label", "optional leading icon", "optional trailing kbd"],
  "sizes": { "default": { "height": "28px", "padding-x": "12px", "font": "compact (12px / 500)", "radius": "md" } },
  "variants": {
    "primary":     { "use": "the one most-likely action per context (Sync, Curate, Apply local)" },
    "default":     { "use": "secondary actions with visible affordance" },
    "ghost":       { "use": "tertiary actions, header chrome (Filter, Cancel, Import)" },
    "destructive": { "use": "irreversible (Wipe, Delete, Cancel sync)" },
    "icon":        { "use": "single-glyph control (eject, close)", "size": "28×28" }
  },
  "states": {
    "primary": {
      "rest":     { "bg": "accent",  "fg": "bg",   "border": "transparent" },
      "hover":    { "bg": "#ffffff", "fg": "bg",   "border": "transparent" },
      "disabled": { "opacity": 0.4 }
    },
    "default": {
      "rest":     { "bg": "transparent", "fg": "text", "border": "border-hi" },
      "hover":    { "bg": "elev-hi",     "fg": "text", "border": "rgba(255,255,255,0.18)" },
      "disabled": { "opacity": 0.4 }
    },
    "ghost": {
      "rest":     { "bg": "transparent", "fg": "muted",   "border": "transparent" },
      "hover":    { "bg": "hover",       "fg": "text",    "border": "transparent" },
      "disabled": { "opacity": 0.4 }
    },
    "destructive": {
      "rest":     { "bg": "transparent",            "fg": "danger", "border": "rgba(248,113,113,0.20)" },
      "hover":    { "bg": "rgba(248,113,113,0.10)", "fg": "danger", "border": "rgba(248,113,113,0.32)" },
      "disabled": { "opacity": 0.4 }
    },
    "icon": "inherits ghost states; container is square 28×28; icon at md (13px); fill currentColor"
  },
  "rules": [
    "MUST preserve fg color on hover for all variants — never let primary fg drift to text-mid on white bg.",
    "MUST NOT use destructive variant for non-destructive actions to draw attention.",
    "Trailing kbd uses kbd component, with inverted colors when inside primary."
  ]
}
```

### 2.2 kbd (keyboard hint)

```json
{
  "id": "kbd",
  "purpose": "Inline visual for a keyboard key.",
  "size": { "min-w": "18px", "height": "18px", "padding-x": "5px", "radius": "sm" },
  "tokens": { "bg": "elev", "fg": "muted", "border": "border-hi", "font": "10px mono / 500" },
  "rules": ["Never used as an interactive element — it's a label only."]
}
```

### 2.3 input (text)

```json
{
  "id": "input",
  "size": { "height": "28px", "padding-x": "10px", "radius": "md" },
  "tokens": { "bg": "bg", "fg": "text", "border": "border-hi", "placeholder": "faint", "font": "12.5px / 400" },
  "states": {
    "rest":  { "border": "border-hi" },
    "focus": { "border": "rgba(255,255,255,0.30)", "outline": "none" }
  },
  "variants": {
    "search": "leading 12px search icon, padding-left 26px",
    "numeric": "right-aligned, mono font"
  }
}
```

### 2.4 checkbox / toggle / radio

```json
{
  "checkbox": {
    "size": "14×14, radius sm",
    "rest":  { "bg": "bg", "border": "border-hi", "check-icon": "hidden" },
    "hover": { "border": "rgba(255,255,255,0.22)" },
    "on":    { "bg": "accent", "border": "accent", "check-icon": "visible, stroke=bg" },
    "on-hover": { "bg": "#ffffff" }
  },
  "toggle": {
    "size": "track 26×14, knob 10×10",
    "off":  { "track-bg": "elev-hi", "border": "border-hi", "knob": "muted" },
    "on":   { "track-bg": "accent", "border": "accent", "knob": "bg, translate +12px" },
    "transition": "transform 120ms, bg 120ms"
  },
  "radio": {
    "size": "14×14, circle",
    "off":  { "border": "border-hi", "dot": "transparent" },
    "on":   { "border": "accent",    "dot": "accent (6×6)" }
  }
}
```

### 2.5 segmented-control

```json
{
  "id": "segmented",
  "anatomy": "row of buttons inside a 2px-padded shell with bg=bg, border=border-hi, radius=md",
  "item": {
    "rest": { "bg": "transparent", "fg": "muted", "padding": "4px 10px", "font": "11.5px / 500" },
    "hover":{ "bg": "hover", "fg": "text" },
    "on":   { "bg": "elev-hi", "fg": "text" }
  },
  "rules": ["Exactly one item is `on` at a time."]
}
```

### 2.6 slider

```json
{
  "id": "slider",
  "size": { "height": "18px", "track-h": "2px", "thumb": "12×12 circle" },
  "tokens": { "track": "elev-hi", "fill": "text-mid", "thumb": "text", "thumb-hover": "#ffffff" }
}
```

### 2.7 stars (rating)

```json
{
  "id": "stars",
  "purpose": "0–5 rating display & input.",
  "anatomy": "5 star glyphs in a row, gap 1px",
  "size": "12×12 per star, fill currentColor",
  "states": {
    "filled":  { "color": "text-mid" },
    "empty":   { "color": "faint" },
    "hover-anywhere": { "all-stars-color": "text" }
  },
  "rules": [
    "No outer border, no padding, no spacing beyond 1px gap.",
    "Filled and empty are the same glyph in different colors — not stroke vs fill."
  ]
}
```

### 2.8 color-picker (CDJ tag)

```json
{
  "id": "color-picker",
  "purpose": "Pick one of 7 CDJ colors, or none.",
  "items": "1 'none' chip + 7 color chips, each 14×14 radius sm",
  "states": {
    "rest":     { "border": "transparent" },
    "hover":    { "transform": "scale(1.1)" },
    "selected": { "border": "text", "inner-shadow": "inset 0 0 0 1px bg" }
  },
  "none-chip": { "border": "border-hi", "icon": "× at faint, hover text-mid" }
}
```

### 2.9 sidebar-row

```json
{
  "id": "sb-row",
  "size": { "height": "26px (default), 24px (compact)", "padding-x": "14px", "margin-x": "6px", "radius": "sm" },
  "anatomy": ["leading icon|digit|dot", "label", "trailing count"],
  "states": {
    "rest":    { "bg": "transparent", "fg": "text-mid" },
    "hover":   { "bg": "hover",       "fg": "text" },
    "active":  { "bg": "select",      "fg": "text" },
    "drag-over": { "bg": "elev",       "fg": "text", "outline": "1px solid border-hi" }
  },
  "leading": {
    "icon":  { "size": "13px, opacity 0.5, currentColor" },
    "digit": { "shape": "18×18 radius sm, bg elev, border border, fg text-mid; in active state bg=elev-hi fg=text" },
    "dot":   { "size": "6×6 circle; on=status-online, off=status-offline" }
  },
  "count": { "font": "10.5px mono", "fg": "faint", "in-active": "muted" }
}
```

### 2.10 sidebar-section

```json
{
  "id": "sb-section",
  "anatomy": ["head (label + optional + button + optional collapse chevron)", "rows"],
  "head": {
    "padding": "8px 14px 6px",
    "label": "label-sm token (mono 9.5px uppercase, color faint)",
    "+button": "ghost button at 13px, color muted → text on hover"
  },
  "between-sections": "1px border-top with 4px margin-top + 8px padding-top"
}
```

### 2.11 panel-header

```json
{
  "id": "panel-header",
  "size": { "height": "40px (regular), 38px (in-app dense)" , "padding-x": "14px", "radius": "md" },
  "tokens": { "bg": "surface", "border": "border" },
  "anatomy": {
    "left":  "title (h2) + subtitle (caption muted) — OR — breadcrumb — OR — target-chip",
    "right": "action buttons, primary action rightmost"
  },
  "variants": {
    "library":         "title + subtitle, ghost actions",
    "playlist-source": "title + subtitle, Filter ghost + Curate primary",
    "usb-source":      "title + subtitle (mounted/synced), Eject ghost + Wipe destructive + Sync primary",
    "filtering":       "left collapses to filter-input component (full width), right hides",
    "in-target-mode":  "left shows target-chip with × close, right shows mode-specific primary"
  }
}
```

### 2.12 filter-input

```json
{
  "id": "filter-input",
  "purpose": "Inline filter that replaces panel header content on ⌘F.",
  "size": { "height": "26px", "padding-x": "10px", "radius": "sm" },
  "tokens": { "bg": "bg", "border": "border-hi" },
  "anatomy": ["leading 12px search icon (faint)", "input (no border)", "count (mono 10.5px muted)", "trailing × (muted → text on hover)"]
}
```

### 2.13 track-row

```json
{
  "id": "track-row",
  "size": { "height": "28px (default), 26px (compact)", "padding-x": "12px (default), 10px (compact)" },
  "grid": "26px 2.4fr 1.6fr 1.2fr 60px 44px 38px 50px (default columns: # title artist label bpm-key time tag color)",
  "states": {
    "rest":     { "bg": "transparent or rgba(255,255,255,0.02) on even rows" },
    "hover":    { "bg": "hover" },
    "selected": { "bg": "select" },
    "multi":    { "bg": "select" },
    "analyzing":{ "opacity": 0.45, "bpm/time/meta": "show — at faint" },
    "drag-source":{ "opacity": 0.5 },
    "drag-over-row": "show drop-line above or below; row itself unchanged"
  },
  "cells": {
    "num":   { "fg": "faint", "font": "10.5px mono tabular, text-align: right" },
    "title": { "fg": "text",  "font": "compact 12px" },
    "artist":{ "fg": "text-mid" },
    "label": { "fg": "muted" },
    "bpm-key": { "fg": "text-mid", "font": "11px mono tabular", "format": '"128 · 8A"' },
    "time":  { "fg": "text-mid", "font": "11px mono tabular" },
    "tag":   "1+ digit-pills (14×14, bg elev-hi, fg text-mid, font 9px mono)",
    "color": "single 9×9 radius xs, fill = track-color-N"
  }
}
```

### 2.14 column-header-row

```json
{
  "id": "col-h-row",
  "size": { "height": "28px", "padding-x": "14px", "column-gap": "16px" },
  "label-style": "label-sm token (mono 9.5px uppercase faint)",
  "states": {
    "rest":    { "fg": "faint" },
    "hover":   { "fg": "text-mid" },
    "sorted":  { "fg": "text", "trailing-arrow": "▲ asc / ▼ desc at 8px opacity 0.7" },
    "dragging":{ "fg": "text", "bg": "elev", "padding-x": "8px", "radius": "sm" }
  },
  "rules": ["Number columns get min-width 80px so values like 'BPM' don't collide with neighbors."]
}
```

### 2.15 detail-pane

```json
{
  "id": "detail-pane",
  "purpose": "Right panel content in detail mode.",
  "size": { "width": "320px (regular), ~280px (compact app)", "padding": "14px" },
  "tokens": { "bg": "surface", "border": "border", "radius": "md" },
  "modes": {
    "single":  ["art-placeholder", "title (h2)", "artist (caption text-mid)", "meta-list", "section: Cue points", "section: Comment", "section: File path"],
    "multi":   ["count-display (display token, centered)", "label 'tracks selected'", "meta-list (totals)", "bulk-actions"],
    "empty":   ["icon (faint)", "label 'No track selected' (text-mid)", "sublabel (muted)"],
    "missing": ["dimmed art (opacity 0.5)", "title (text-mid)", "artist", "warn-alert", "meta with em-dashes", "actions: Locate file, Reanalyze, Remove"]
  },
  "art-placeholder": { "size": "aspect 1:1, full width", "bg": "elev", "radius": "sm", "icon": "ic-note at 50% size, color faint" },
  "meta-list": {
    "format": "dl with grid 96px 1fr, gap 6px 12px, align-items baseline",
    "dt": "label token (mono 10px uppercase faint), nowrap",
    "dd": "12px text, nowrap with text-overflow ellipsis (use .wrap modifier for paths)"
  },
  "cue-list-item": "grid 14px 1fr 50px (pip · name · pos), height ~24px, hover bg=hover",
  "warn-alert": "padding 8px 10px, bg rgba(248,113,113,0.07), border 1px rgba(248,113,113,0.20), radius sm, color danger, font 11.5px"
}
```

### 2.16 menu (context / dropdown)

```json
{
  "id": "menu",
  "size": { "min-width": "180px", "padding": "4px", "radius": "lg" },
  "tokens": { "bg": "elev", "border": "border-hi", "shadow": "menu" },
  "item": {
    "size": { "padding": "5px 10px", "radius": "sm", "font": "12px" },
    "anatomy": ["label", "optional trailing kbd-shortcut (mono 10px faint)"],
    "states": {
      "rest":    { "fg": "text-mid" },
      "hover":   { "bg": "select",  "fg": "text" },
      "danger":  { "fg": "danger" },
      "danger-hover": { "bg": "rgba(248,113,113,0.10)", "fg": "danger" },
      "disabled": { "fg": "faint" }
    }
  },
  "separator": "1px border-color, margin 4px 0"
}
```

### 2.17 dialog

```json
{
  "id": "dialog",
  "size": { "min-width": "340px", "padding": "20px 22px", "radius": "xl" },
  "tokens": { "bg": "elev", "border": "border-hi", "shadow": "dialog" },
  "anatomy": ["title (h2)", "body (caption text-mid, line-height 1.5)", "actions row (right-aligned, gap 6px)"],
  "variants": ["text-input", "confirm (Cancel ghost + primary or destructive)"]
}
```

### 2.18 toast

```json
{
  "id": "toast",
  "size": { "padding": "10px 14px", "radius": "lg" },
  "tokens": { "bg": "elev", "border": "border-hi", "shadow": "toast" },
  "anatomy": ["leading 14px icon", "message (12px text)", "optional sub-message (11px muted)", "optional Undo button"],
  "variants": {
    "default": { "icon-color": "text-mid" },
    "success": { "icon-color": "status-online" },
    "error":   { "icon-color": "danger", "border": "rgba(248,113,113,0.32)" }
  }
}
```

### 2.19 inline-alert

```json
{
  "id": "inline-alert",
  "size": { "padding": "10px 12px", "radius": "md" },
  "anatomy": ["leading icon (flex-shrink 0)", "body (line-height 1.4, strong = text)"],
  "variants": {
    "info": { "bg": "elev",                         "border": "border",                        "fg": "text-mid", "icon-fg": "text-mid" },
    "warn": { "bg": "rgba(248,113,113,0.06)",       "border": "rgba(248,113,113,0.18)",        "fg": "text-mid", "icon-fg": "danger" }
  }
}
```

### 2.20 player-compact

```json
{
  "id": "player-compact",
  "purpose": "Always-visible bottom player, full window width.",
  "size": { "padding": "10px 14px", "grid": "40px 28px 1fr (art · play · meta+wf)", "gap": "12px" },
  "tokens": { "bg": "bg", "border-top": "border" },
  "art": "40×40, radius sm, bg elev, ic-note at 22px color faint",
  "play": "28×28 circle, bg accent, fg bg, play triangle 10×10",
  "meta-row": "title (12px / 500) + artist (11px muted)",
  "waveform": "28px tall, achromatic spectral bars (bass/mid/high in white→grey gradient based on played %), playhead = 1px accent",
  "times": "mono 10px faint, justify between"
}
```

### 2.21 player-expanded

```json
{
  "id": "player-expanded",
  "purpose": "Track editor, replaces compact player on ⇧⇥.",
  "anatomy": ["head", "cuestrip", "waveform-region", "beat-grid", "controls"],
  "head": "play button + title + artist + meta-pills (BPM·Key·Time) + segmented (Color/Mono) + Save/Discard ghost + collapse icon",
  "cuestrip": "18px tall, cue markers as 1px vertical lines with label above (mono 9px text-mid)",
  "waveform-region": "120px tall, placeholder allowed, playhead = 1px text vertical line",
  "beat-grid": "14px tall, beats as 1px borders, bars as taller 10px lines with bar number above (mono 8.5px faint)",
  "controls": "left: time mono 11px text-mid · center: nudge controls for BPM/Key + ½× 2× 'Set first beat' ghost buttons · right: 'Add cue' default button"
}
```

### 2.22 nudge

```json
{
  "id": "nudge",
  "purpose": "Numeric editor with − / value / + buttons.",
  "size": { "height": "22px", "radius": "sm" },
  "tokens": { "bg": "bg", "border": "border-hi" },
  "anatomy": ["−button (22×22 ghost)", "value (padding-x 8px, mono 11px text, between-borders)", "+button"],
  "states": { "button-hover": { "bg": "hover", "fg": "text" } }
}
```

### 2.23 status-bar

```json
{
  "id": "status-bar",
  "size": { "height": "24px (in-app), 26px (standalone with border)", "padding-x": "12px", "font": "10.5–11px" },
  "tokens": { "bg": "bg", "border-top": "border", "fg": "muted" },
  "left-region": ["spinner (optional)", "current-action message", "separators (· faint)", "context message"],
  "right-region": ["progress-bar (optional)", "library count summary"]
}
```

### 2.24 progress-bar

```json
{
  "id": "progress",
  "size": { "height": "4px", "radius": "xs" },
  "tokens": { "track": "elev", "fill": "text" },
  "variants": {
    "determinate":   "fill width = progress %",
    "indeterminate": "fill width 30%, animation translateX(-100%) → 450% over 1.6s ease-in-out infinite, fill = text-mid",
    "thin":          "height 2px (status-bar inline)"
  }
}
```

### 2.25 spinner

```json
{
  "id": "spinner",
  "size": { "default": "11×11", "small": "9×9" },
  "style": "1.4px border (1.2px small), border-color border-hi, border-top-color text-mid, border-radius 50%, animation rotate 1s linear infinite"
}
```

### 2.26 tag-chip / digit-pill / target-chip

```json
{
  "tag-chip": {
    "use": "inline tag display (filter, breadcrumb context)",
    "size": "padding 2px 7px 2px 4px, radius sm",
    "tokens": { "bg": "elev", "border": "border-hi", "fg": "text-mid", "font": "11px" },
    "anatomy": ["digit (14×14, bg=bg, fg=text, mono 9.5px)", "name"]
  },
  "digit-pill": {
    "use": "track-row tag column",
    "size": "14×14, radius xs, mono 9px / 500",
    "tokens": { "bg": "elev-hi", "fg": "text-mid" }
  },
  "target-chip": {
    "use": "target panel header indicator",
    "size": "padding 3px 8px 3px 6px, radius sm",
    "tokens": { "bg": "elev", "border": "border-hi", "fg": "text" },
    "anatomy": ["pin icon (10px text-mid)", "name", "× close (muted → text on hover)"]
  }
}
```

### 2.27 command-palette

```json
{
  "id": "cp",
  "purpose": "⌘K overlay — search tracks/playlists/USBs/actions.",
  "container": { "max-width": "560px", "bg": "elev", "border": "border-hi", "radius": "xl", "shadow": "palette", "overflow": "hidden" },
  "input-row": { "padding": "12px 14px", "border-bottom": "border", "leading-icon": "14px faint", "input": "transparent 14px text", "trailing-kbd": "esc" },
  "section": { "head": "padding 6px 14px, label-sm token; rows separated by 1px border-top between sections" },
  "row": {
    "size": "padding 7px 14px, gap 10px",
    "anatomy": ["icon (14px muted)", "name (12.5px text)", "sub (11.5px muted, inline after name)", "right (10px mono faint, e.g. ⏎ or shortcut)"],
    "states": { "rest": "transparent", "hover|active": "bg select" }
  },
  "footer": "padding 8px 14px, border-top border, bg=bg, font 10.5px faint, left=key hints, right=result count"
}
```

### 2.28 ghost (drag) + drop-line

```json
{
  "drag-ghost": {
    "size": "padding 6px 10px, radius md",
    "tokens": { "bg": "elev", "border": "border-hi", "shadow": "toast", "fg": "text", "font": "12px" },
    "multi-form": "leading badge (min-w 18px, bg accent, fg bg, mono 10px / 600) + 'tracks' label"
  },
  "drop-line": {
    "shape": "1px horizontal line, color accent",
    "marker": "6×6 circle at left end, color accent",
    "placement": "between two rows during drag"
  },
  "drop-overlay-panel": {
    "use": "panel-as-target during drag",
    "size": "padding 32px, radius xl",
    "tokens": { "bg": "surface", "border": "1.5px dashed rgba(255,255,255,0.20)", "fg": "text-mid" },
    "title": "14px / 500 text",
    "sub": "11.5px muted"
  }
}
```

### 2.29 settings-page

```json
{
  "id": "settings",
  "layout": "grid 200px 1fr (nav + pane), min-height 420px, bg=bg, border=border, radius=md",
  "nav": {
    "padding": "14px 0",
    "section-label": "label-sm at padding 8px 16px 4px",
    "item": "padding 7px 16px, font 12.5px",
    "states": { "rest": "fg text-mid", "hover": "bg hover fg text", "active": "bg select fg text" }
  },
  "pane": { "padding": "22px 28px", "h": "16px / 500 / -0.01em tracking", "sub": "12px muted, 18px margin-bottom" },
  "field": {
    "layout": "grid 1fr auto, gap 8px 16px, padding 12px 0, border-top border",
    "anatomy": ["label (12.5px text)", "desc (11.5px muted)", "control (toggle/seg/input/etc. — spans both rows)"]
  }
}
```

### 2.30 onboarding

```json
{
  "id": "onboard",
  "layout": "centered column, padding 56px 24px, bg=bg, border=border, radius=xl, gap 14px",
  "anatomy": ["wordmark (display token)", "message (13px text-mid, max 380px)", "actions (primary + default)", "shortcut hint (11px faint)"]
}
```

### 2.31 conflict-diff (USB drift)

```json
{
  "id": "conflict",
  "header": "padding 12px 14px, border-bottom border; left: title h2 + sub 11.5px muted; right: action buttons (Discard ghost + Keep both default + Apply local primary)",
  "row": {
    "layout": "grid 22px 50px 1fr 80px, gap 10px, padding 6px 14px, border-top border",
    "marker": "mono 10px / 500, color depends on type",
    "label": "mono 10px uppercase muted",
    "name": "12px text, sub-info inline at muted",
    "trailing-source": "11px muted"
  },
  "types": {
    "add":     { "marker": "+", "color": "status-online" },
    "remove":  { "marker": "−", "color": "danger" },
    "changed": { "marker": "~", "color": "status-warn" }
  },
  "row-hover": "bg hover"
}
```

### 2.32 bulk-action-bar

```json
{
  "id": "bulk-bar",
  "purpose": "Persistent strip while multi-select active.",
  "size": "padding 8px 14px, radius md",
  "tokens": { "bg": "elev", "border": "border-hi" },
  "left": ["count (12px text / 500)", "separator", "summary (12px text-mid)"],
  "right": ["ghost actions (Tag, Set color)", "default action (Add to playlist…)", "Clear selection ghost (with esc kbd)"]
}
```

### 2.33 empty-state

```json
{
  "id": "empty",
  "size": "padding 36px 24px, text-align center, radius md",
  "tokens": { "bg": "surface", "border": "1px dashed border-hi" },
  "anatomy": ["title (13px / 500 text-mid)", "sub (11.5px muted)"]
}
```

---

## 3. Layout patterns

### 3.1 App shell

```
┌──────────────────────────────────────────┐
│ titlebar (28px)                          │
├──────────────────────────────────────────┤
│ sidebar │ list panel │ right panel       │  ← main area, flex 1
│ 220px   │ flex       │ 280–320px         │
├──────────────────────────────────────────┤
│ player (compact 60px or expanded ~40vh)  │
├──────────────────────────────────────────┤
│ status-bar (24px)                        │
└──────────────────────────────────────────┘
```

- Player and status-bar span full window width including over the sidebar.
- All column-splitting happens inside main area only.
- Sidebar: bg, border-right border. List panel: surface, border-right border. Right panel: surface, no border-right.

### 3.2 Right panel modes

| Mode | Contents | When | Width |
|---|---|---|---|
| detail | detail-pane spec 2.15 | default (browse, triage) | 280–320px |
| target | panel-header (target variant) + track-list | curate or sync | 280–360px |
| hidden | — | manual focus mode | 0 |

Transitions are instant (no animation). Header morphs in place.

### 3.3 Responsive thresholds

```json
{
  "breakpoints": {
    ">=1100": "all three columns visible",
    "900-1099": "right panel auto-collapses",
    "800-899":  "sidebar collapses to icon-only",
    "<800": "undefined / wrong device"
  }
}
```

### 3.4 Discoverability

```json
{
  "rules": [
    "Primary actions live in panel headers as labeled buttons.",
    "Secondary actions are visible on row hover or in panel toolbars.",
    "Right-click menus mirror panel actions — never the only path.",
    "Keyboard shortcuts are surfaced in tooltips and the command palette."
  ]
}
```

---

## 4. Implementation notes for agents

When asked to implement a screen or component:

1. **Lookup tokens by name.** Never inline raw hex, sizes, or radii — pull from §1.
2. **Match the spec exhaustively.** If a state isn't in the component spec, it doesn't exist. Don't invent hover/focus styles.
3. **Use existing components.** A new requirement should be expressed by composing existing components (panel-header + track-list + bulk-action-bar) before introducing new ones.
4. **Hover preserves foreground.** Buttons, menu items, and any colored element MUST keep their fg on hover unless the spec says otherwise.
5. **Mono type for tabular numbers, paths, kbd, labels — nothing else.**
6. **Chrome stays achromatic.** Color only enters via track-color-N (CDJ tags), the danger token, and status-* tokens. Accent (#fafafa) is white-on-dark, not a chromatic color.
7. **Icons inherit currentColor.** Containers set the color, icons follow.
8. **Single source of truth for placeholders.** Cover placeholder = ic-note SVG, never an emoji or character glyph.

---

## 5. Open / pending

- `detail-pane` content for "track has unresolved analysis" — covered as missing-file variant; review for v2.
- `track-editor` (player-expanded) cue management UX — list view vs inline-on-waveform — undecided.
- `conflict-diff` interaction (per-row resolve) — currently bulk-only; may need per-row keep/discard.
- `command-palette` action vocabulary — start with track + playlist results; expand actions in v2.
- `light theme` — explicitly out of scope until core flow is solid.
