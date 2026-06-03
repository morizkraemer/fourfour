import './icon.css';

/*
 * Icon primitive. Renders a lucide-style 24×24 stroke glyph as inline SVG.
 * stroke is `currentColor`, so the parent's text color drives the icon color
 * (matches how the design-system rows tint their leading icons).
 */

const NS = 'http://www.w3.org/2000/svg';

// Inner SVG markup per glyph (lucide geometry, 24×24 viewBox).
const ICONS = {
  list: '<path d="M8 6h13"/><path d="M8 12h13"/><path d="M8 18h13"/><path d="M3 6h.01"/><path d="M3 12h.01"/><path d="M3 18h.01"/>',
  'clock-3': '<circle cx="12" cy="12" r="10"/><path d="M12 6v6h4.5"/>',
  plus: '<path d="M5 12h14"/><path d="M12 5v14"/>',
  'chevron-down': '<path d="m6 9 6 6 6-6"/>',
  'chevron-right': '<path d="m9 18 6-6-6-6"/>',
  search: '<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>',
  x: '<path d="M18 6 6 18"/><path d="m6 6 12 12"/>',
  eject: '<path d="M5 16 12 6l7 10z"/><path d="M5 19h14"/>',
  disc: '<circle cx="12" cy="12" r="10"/><circle cx="12" cy="12" r="2"/>',
  star: '<path d="M11.48 3.5a.6.6 0 0 1 1.04 0l2.45 4.97 5.48.8a.6.6 0 0 1 .33 1.02l-3.96 3.86.94 5.46a.6.6 0 0 1-.87.63L12 17.98l-4.9 2.57a.6.6 0 0 1-.87-.63l.94-5.46-3.97-3.86a.6.6 0 0 1 .33-1.02l5.48-.8z"/>',
};

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

export const ICON_NAMES = Object.keys(ICONS);
