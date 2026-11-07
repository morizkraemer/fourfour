# Viewport Authoring

Use this skill when creating or editing UI elements inside this project's `viewport/` folder.

## Rules

- Build in three layers: `primitive`, `module`, then `composite`.
- Import and call existing primitives. Do not re-implement their markup in modules or composites.
- Style through CSS custom properties and tokens. Do not hard-code colors, spacing, radii, typography, or motion values inside repeated components.
- Keep project content under `viewport/`: artboards in `viewport/artboards/*.artboard.js`, shared UI in `viewport/design-system/`, config in `viewport.config.js`.
- Every artboard exports `meta` and a default `render()` function.
- `meta.layer` must be one of the configured layers.
- `render()` returns one DOM element.

## Artboard Shape

```js
export const meta = { title: 'Button', layer: 'primitive' };

export default function render() {
  return Button({ children: 'Save' });
}
```

## Minimal Cascade Example

Token:

```css
:root {
  --control-bg: var(--ff-elev);
  --control-radius: var(--ff-radius-md);
}
```

Primitive:

```js
import { el } from './index.js';

export function Button({ children }) {
  return el('button', {
    style: {
      background: 'var(--control-bg)',
      borderRadius: 'var(--control-radius)',
    },
  }, [children]);
}
```

Module:

```js
import { Button } from '../design-system/button.js';
import { el } from '../design-system/index.js';

export function Toolbar() {
  return el('div', {}, [
    Button({ children: 'Save' }),
    Button({ children: 'Export' }),
  ]);
}
```

Composite:

```js
import { Toolbar } from './toolbar.js';
import { el } from '../design-system/index.js';

export function EditorShell() {
  return el('main', {}, [Toolbar()]);
}
```

Artboard:

```js
import { EditorShell } from '../design-system/editor-shell.js';

export const meta = { title: 'Editor Shell', layer: 'composite' };

export default function render() {
  return EditorShell();
}
```

When a primitive changes, Vite HMR updates every artboard that imports it while Viewport keeps canvas pan, zoom, and card positions intact.

## Comments

The user comments on cards in the canvas, usually from a phone; the comments land in `viewport/comments.json`, keyed by artboard id: `{ id, text, at, resolved, by? }`.

**Every turn, run `npx viewport comments` first** and answer or act on every comment it prints, the same way you handle a user message. It lists only comments you have not seen (board id, author, time, the thread), then marks them read. `--peek` looks without marking, `--all` prints everything.

- `npx viewport comments reply <board> "<text>"` answers on that board (written as `by: "agent"`). Say what you changed or what you need decided.
- `npx viewport comments resolve <board> <comment id>` after you acted on one.
- Open comments are feedback on that card's file (the id maps to the file as above). Never delete the user's comments; keep `comments.json` valid JSON.
- A key `<artboard id>#<name>` is a comment on one variation inside that card: the element with `data-variant="<name>"`. Give every variation in a multi-variation card a stable `data-variant` name on its outermost element so the user can comment on it alone.

### Review workflow

- The user reviews a whole page at once by commenting on its cards and variations, then tells you which page's comments to act on.
- Act on that page's open comments. The next round replaces the commented boards with new versions; delete the old boards, do not keep them. Git history keeps them.
- Commented boards stay visible. There are no done or reviewed marks.

