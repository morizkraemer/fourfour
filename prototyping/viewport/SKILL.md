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
