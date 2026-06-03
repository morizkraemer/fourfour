import './color-swatch.css';
import { el } from '../utils/dom.js';

/*
 * CDJ color swatch. Rounded square color tag shown on track rows (§7).
 *   color: CDJ index 1..7 or any hex
 *   size: edge length in px (default 9, matching the track-row tag slot)
 */
export function createColorSwatch({ color, size = 9 } = {}) {
  const fill = typeof color === 'number' ? `var(--ff-track-color-${color})` : color;
  return {
    element: el('span', {
      class: 'ff-swatch',
      style: { width: `${size}px`, height: `${size}px`, background: fill },
    }),
  };
}
