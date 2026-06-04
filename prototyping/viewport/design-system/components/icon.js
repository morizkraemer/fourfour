import './icon.css';
import { ICONS, ICON_NAMES } from './icon-glyphs.js';

/*
 * Icon primitive. Renders a lucide-style 24×24 stroke glyph as inline SVG.
 * stroke is `currentColor`, so the parent's text color drives the icon color
 * (matches how the design-system rows tint their leading icons).
 * Glyph geometry lives in icon-glyphs.js (shared with Icon.svelte).
 */

const NS = 'http://www.w3.org/2000/svg';

export function createIcon({ name, size = 14, strokeWidth = 2 } = {}) {
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('class', 'ff-icon');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('width', String(size));
  svg.setAttribute('height', String(size));
  svg.setAttribute('fill', 'none');
  svg.setAttribute('stroke', 'currentColor');
  svg.setAttribute('stroke-width', String(strokeWidth));
  svg.setAttribute('stroke-linecap', 'round');
  svg.setAttribute('stroke-linejoin', 'round');
  svg.innerHTML = ICONS[name] ?? '';
  return { element: svg };
}

export { ICON_NAMES };
