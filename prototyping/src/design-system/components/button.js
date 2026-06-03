import './button.css';
import { el } from '../utils/dom.js';
import { createIcon } from './icon.js';

/*
 * Button primitive. Variants + states copied 1:1 from design-system frame §4.
 *   variant: 'primary' | 'default' | 'ghost' | 'destructive'
 *   iconOnly: square 28×28 icon button (pair with variant:'ghost' for the catalog look)
 *   icon: a name from icon.js (leading glyph)
 *   disabled: dims to 0.4 and drops pointer events
 * Hover states live in button.css.
 */
export function createButton({
  label,
  variant = 'default',
  icon,
  iconOnly = false,
  disabled = false,
  onClick,
} = {}) {
  const children = [];
  if (icon) children.push(createIcon({ name: icon, size: 14 }).element);
  if (label && !iconOnly) children.push(el('span', { class: 'ff-btn__label', text: label }));

  const classes = ['ff-btn', `ff-btn--${variant}`];
  if (iconOnly) classes.push('ff-btn--icon');
  if (disabled) classes.push('ff-btn--disabled');

  const props = { class: classes.join(' '), type: 'button' };
  if (disabled) props.disabled = 'true';
  if (onClick) props.onClick = onClick;

  return { element: el('button', props, children) };
}
