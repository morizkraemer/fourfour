# prototyping - fourfour UI canvas

An infinite-canvas **visualizer** for fourfour UI mockups. Agents author mockups as
plain JS/CSS artboards; you pan/zoom to review them. Not the production app, but
`viewport/design-system/` is built to lift into the Tauri frontend later.

```bash
npm install
npm run dev      # http://localhost:5180
```

## The three layers (build bottom-up)

1. **Primitive** - one reusable control (button, sidebar-row, track-row, waveform).
   Lives in `viewport/design-system/components/<name>.{js,css}`. Factory
   `createX(props) -> { element, update?, destroy? }`. Styled only with `--ff-*` tokens
   (`viewport/design-system/tokens.css`). Export it from `viewport/design-system/index.js`.
2. **Module** - composes primitives into a self-contained unit (sidebar, track list,
   player). An artboard.
3. **Composite** - composes whole modules into a full-window screen (Browse, Curate).
   An artboard. Adds no internal spacing of its own.

Need a control that doesn't exist? Build the primitive first, then use it. Don't inline
one-off DOM where a primitive belongs.

## The artboard contract

Every `viewport/artboards/**/*.artboard.js` is auto-discovered by the shared `viewport`
package. There is
**no registration step**. Adding an artboard is adding a file.

```js
// viewport/artboards/track-list.artboard.js
import { createTrackRow } from '../design-system/index.js';

export const meta = { title: 'Track List', layer: 'module' }; // primitive | module | composite

export default function render() {
  const el = document.createElement('div');
  // build with design-system primitives + --ff-* tokens
  return el; // the CONTENT element; the canvas wraps it in a titled card
}
```

The path below `viewport/artboards` minus `.artboard.js` is the artboard id. Layer
determines its sidebar group and default canvas column.

## Tokens

`viewport/design-system/tokens.css` is ported 1:1 from `../docs/design_system.md`, which stays
the normative source. Use token vars by name; never hardcode colors/sizes.

## Layout persistence

Default layout is a deterministic grid (one column per layer). Manual drags and camera
position are saved to `localStorage` under the `fourfour:` namespace.
