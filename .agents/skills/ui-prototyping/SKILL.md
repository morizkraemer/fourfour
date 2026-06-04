---
name: ui-prototyping
description: >
  Build fourfour UI in the prototyping/ canvas (design-system primitives → modules → full-screen composites). Use whenever the user wants to add or edit a primitive, an artboard/module, or assemble a full screen in prototyping/, or says things like "build a new artboard", "add a track row", "put the Browse screen together", "make a module out of these primitives". Enforces layering, spacing, the file-based auto-registry, and verification rules that prevent overflow, double-spacing, and accidental module rewrites.
---

# UI Prototyping (fourfour canvas system)

A three-layer, bottom-up system for prototyping the fourfour DJ-library UI. You **always** build from the bottom up and **never** skip a layer. Most mistakes come from mixing layers — building a one-off where a primitive belongs, or adding spacing in a composite that the module already owns.

The canvas is a **visualizer**: a Konva-backed infinite pannable surface where each `*.artboard.js` renders as a titled card. It is not the production app — but `src/design-system/` is built to lift into the Tauri frontend unchanged, so primitives must stay framework-agnostic vanilla JS/CSS.

## Repo map

| Thing | Location |
|---|---|
| Primitives | `prototyping/src/design-system/components/<name>.{js,css}` |
| Design-system barrel | `prototyping/src/design-system/index.js` (re-exports primitives + utils) |
| Design tokens | `prototyping/src/design-system/tokens.css` (`--ff-*`), ported 1:1 from `docs/design_system.md` (normative) |
| DOM helper | `prototyping/src/design-system/utils/dom.js` → `el(tag, props, children)` |
| Artboards (modules + composites) | `prototyping/src/artboards/**/*.artboard.js` |
| Canvas engine | `prototyping/src/canvas/{canvas,artboard,sidebar,store,layout}.js` |
| Build / dev | `cd prototyping && npm run build` / `npm run dev` (picks first free port from 5180) |

## The artboard contract — THE difference from other canvas systems

There is **no registration step**. `src/main.js` discovers every `src/artboards/**/*.artboard.js` via `import.meta.glob`. Adding an artboard = adding a file. Never edit `canvas.js` to "register" anything.

```js
// src/artboards/track-row.artboard.js
import { createTrackRow } from '../design-system/index.js';

export const meta = { title: 'Track Row', layer: 'primitive' }; // layer: primitive | module | composite

export default function render() {
  // build with design-system primitives + --ff-* tokens
  return el('div', {}, [ /* ... */ ]); // return the CONTENT element; the canvas wraps it in a titled card
}
```

- The filename minus `.artboard.js` is the artboard **id**.
- `meta.layer` drives the sidebar group **and** the default canvas column. Set it correctly.
- `render()` returns the bare content element. **Do not** call any card/artboard wrapper yourself — `createArtboard` in the engine does that and adds the live-dims header.

## The three layers

1. **Primitive** — one reusable control (button, sidebar-row, track-row, panel-header, waveform). Factory `createX(props) → { element, update?, destroy? }`. Owns its own visuals + behavior + CSS. Class prefix `ff-`. Styled only with `--ff-*` tokens. Lives in `design-system/components/`, exported from `design-system/index.js`.
2. **Module (artboard)** — composes primitives into one self-contained unit of a fixed natural size (a sidebar, a track list, the global player). Owns its internal padding/gap/width in its own scoped CSS (unique class prefix). `meta.layer = 'module'`.
3. **Full-screen composite (artboard)** — composes **whole modules** into a window-sized screen (Browse, Curate, Settings, Player). Adds **zero** internal spacing. `meta.layer = 'composite'`.

## Golden rule table

| # | Rule | Why / what breaks without it |
|---|---|---|
| 1 | **Build bottom-up.** Need a control that doesn't exist? Build the primitive first, then use it. | Prevents one-off DOM/CSS that diverges from the system. |
| 2 | **Reuse, never reinvent.** Before writing markup, check `design-system/components/` for a primitive and `artboards/` for a module. | The point is one robust system, not copies. |
| 3 | **Composites compose existing modules AS-IS.** Never invent or edit a module while assembling a composite. | Editing a module to fit a composite breaks its standalone artboard + every other user. |
| 4 | **Modules own their spacing. Composites add NONE.** No `gap`/`padding`/`margin` on composite wrapper slots — modules already include theirs. | Double-spacing. |
| 5 | **Never force a composite narrower than its modules' combined natural width.** Let it shrink-wrap (`display:inline-flex; width:fit-content`) OR pin to the real window frame with `overflow:hidden` + flattened slots. | A too-small fixed width collapses columns and overlaps modules. |
| 6 | **To reuse part of a module** (just its header, just one panel), **export a builder** from the module — don't duplicate its markup. | One source of truth; the standalone artboard keeps working. |
| 7 | **No registration — adding a file IS registering.** Put the `*.artboard.js` in `src/artboards/` with correct `meta`. Never touch `canvas.js`. | The auto-registry is the system; editing the engine to register is the #1 wrong move. |
| 8 | **Styling is tokens-only.** Every color/size/space/radius/font comes from a `--ff-*` token. No hardcoded hex/px for themed values. | Tokens are the contract that lifts into Tauri; hardcoding breaks theming. |
| 9 | **Verify every layer**: `npm run build` transforms with no errors, then look at the card before composing upward. | Catches overflow/clipping before it compounds two layers up. |

## What you MAY freely change (the play area)

Spacing, padding, gaps, sizes, layout, grid/flex arrangement, token choices, colors **within a single layer's own CSS file**. The structure is fixed; the formatting is yours to tune. Keep each change inside the layer that owns it (rule 4).

## Building a primitive

- New file pair `<name>.{js,css}` in `src/design-system/components/`.
- First line of the `.js`: `import './<name>.css';`.
- Factory: `export function createX(props) { ... return { element, update, destroy }; }` (`update`/`destroy` optional).
- Class prefix `ff-`. Use only `--ff-*` tokens.
- **Export it from `src/design-system/index.js`** (uncomment/add the line) so artboards import it from the barrel.
- The **waveform** primitive is a Canvas-2D renderer (per `docs/ui-architecture.md`), not DOM — wrap a `<canvas>` and draw to it; still expose the `createX → { element, update }` shape.

## Building a module

- New `src/artboards/<name>.artboard.js`. Optional scoped `<name>.css` imported on the first line.
- Import primitives from the barrel: `import { createTrackRow } from '../design-system/index.js';`.
- Compose into a content element with a unique class prefix; that CSS owns the module's **width, padding, gap**.
- `export const meta = { title, layer: 'module' };` and `export default function render() { return contentEl; }`.
- Export reusable internal builders so composites can pull pieces without duplication (rule 6).

## Building a full-screen composite

- New `src/artboards/<name>.artboard.js` importing **module builders** and composing them. `meta.layer = 'composite'`.
- Wrapper CSS:
  - `display:inline-flex; flex-direction:column; width:fit-content;` to shrink-wrap, **or** pin to the real window frame with fixed `width/height` + `overflow:hidden` + slots flattened to `flex:1`.
  - Slot divs are bare `display:flex` — **no gap, no padding** (rule 4).
  - A row of modules: `display:flex; align-items:flex-start;` or a grid whose flexible column is `auto` (never fixed smaller than the module).
- Match the target screens in `docs/ui_vision.md` / the frozen Pencil reference (Browse, Curate, Settings, Player).

## Verification protocol (do every time)

1. `cd prototyping && npm run build` — must show "✓ N modules transformed" and no errors.
2. View the card (the engine prints live `W × H` in its header) and look for: content overflowing its box, modules overlapping, doubled gaps, clipped panels.
3. If it overflows → you violated rule 4 or 5. Remove composite spacing / stop forcing width; let it shrink-wrap.
4. Per project AGENTS.md, bump `VERSION` in `pioneer-usb-writer/src/lib.rs` on UI/tooling changes.

## Gotchas

- The canvas is an infinite pannable surface backed by a **Konva Stage** (`canvas.js`); the DOM overlay (`#world`) is `pointer-events:none` so drag/wheel reach the Stage — `.ff-card` restores `pointer-events:auto`. If a primitive needs to swallow pointer events, it already works inside a card; don't fight the overlay.
- A new artboard lands in its layer's column (auto-grid) and may be off-screen. Click its row in the sidebar to center it; manual drags persist to `localStorage` (key `fourfour:canvas:positions`), and "reset layout" clears them.
- Artboards nest visually: a composite card contains module content (the engine shows one dims header per card).
- `docs/design_system.md` stays the normative token source; `tokens.css` mirrors it. If a token is missing, add it to the doc first, then mirror.

## Token quick reference (`--ff-*`)

Surfaces: `--ff-bg` `--ff-surface` `--ff-elev` `--ff-elev-hi` `--ff-hover` `--ff-select` `--ff-border` `--ff-border-hi`.
Text: `--ff-text` `--ff-text-mid` `--ff-muted` `--ff-faint`. Accent: `--ff-accent`.
Status: `--ff-status-online` `--ff-status-warn` `--ff-status-offline` `--ff-danger`.
CDJ track colors: `--ff-track-color-1` … `--ff-track-color-7`.
Type scale: `--ff-type-display` `--ff-type-h1` `--ff-type-h2` `--ff-type-body` `--ff-type-row` `--ff-type-num` `--ff-type-label` `--ff-type-label-sm` `--ff-type-caption` `--ff-type-small` `--ff-type-compact`.
Fonts: `--ff-font` (Geist), `--ff-font-mono` (Geist Mono).
Spacing: `--ff-space-0` … `--ff-space-11`. Radii: `--ff-radius-xs|sm|md|lg|xl`.
Shadows: `--ff-shadow-menu|dialog|palette|toast`. Motion: `--ff-motion-instant|fast|default|slow` + `--ff-easing`.
