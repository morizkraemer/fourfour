import './button.css';
import { el } from '../utils/dom.js';
import { createIcon } from './icon.js';
import { createKbd } from './kbd.js';

/*
 * Button primitive. Variants + states copied 1:1 from design-system frame §4.
 *   variant: 'primary' | 'default' | 'ghost' | 'destructive'
 *   iconOnly: square 28×28 icon button (pair with variant:'ghost' for the catalog look)
 *   icon: a name from icon.js (leading glyph)
 *   disabled: dims to 0.4 and drops pointer events
 *   kbd: optional trailing keyboard shortcut string
 * Hover states live in button.css.
 */
export function createButton({
  label,
  variant = 'default',
  icon,
  iconOnly = false,
  disabled = false,
  size = 'default',
  kbd,
  onClick,
} = {}) {
  const children = [];
  
  if (icon) {
    let iconSize = 14;
    if (size === 'compact') iconSize = 10;
    else if (size === 'small') iconSize = 13;
    children.push(createIcon({ name: icon, size: iconSize }).element);
  }
  
  if (label && !iconOnly) {
    children.push(el('span', { class: 'ff-btn__label', text: label }));
  }

  if (kbd) {
    children.push(createKbd({ key: kbd }).element);
  }

  const classes = ['ff-btn', `ff-btn--${variant}`];
  if (iconOnly) classes.push('ff-btn--icon');
  if (disabled) classes.push('ff-btn--disabled');
  if (size === 'compact') classes.push('ff-btn--compact');
  if (size === 'small') classes.push('ff-btn--small');

  const props = { class: classes.join(' '), type: 'button' };
  if (disabled) props.disabled = 'true';
  if (onClick) props.onClick = onClick;

  return { element: el('button', props, children) };
}
