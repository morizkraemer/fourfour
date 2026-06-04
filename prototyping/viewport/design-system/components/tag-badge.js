import './tag-badge.css';
import { el } from '../utils/dom.js';

/*
 * Tag / digit badge. The numbered pill used in the sidebar Favorites section
 * (md, outline/filled) and on selected track rows (sm, color fill). From §6 + §7.
 *   variant: 'outline' (rest) | 'filled' (active) | 'color' (CDJ color fill)
 *   size: 'md' (18px, sidebar) | 'sm' (14px, track row)
 *   color: CDJ index 1..7 or any hex (only used by variant 'color')
 */
const SIZES = { md: 18, sm: 14 };

export function createTagBadge({ value, variant = 'outline', color, size = 'md' } = {}) {
  const box = SIZES[size] ?? SIZES.md;
  const style = { width: `${box}px`, height: `${box}px` };
  if (variant === 'color') {
    style.background = typeof color === 'number' ? `var(--ff-track-color-${color})` : color;
  }
  return {
    element: el('span', { class: `ff-badge ff-badge--${variant} ff-badge--${size}`, style }, [
      el('span', { class: 'ff-badge__value', text: String(value) }),
    ]),
  };
}
