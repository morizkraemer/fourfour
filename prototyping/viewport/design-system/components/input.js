import './input.css';
import { el } from '../utils/dom.js';

/*
 * Text input primitive.
 *   variant: 'default' | 'search' | 'numeric'
 *   placeholder: placeholder text
 *   value: initial value
 *   type: HTML input type (default 'text')
 *
 * The search variant wraps the <input> in a div with a leading search icon.
 * The numeric variant right-aligns text and uses the mono font.
 */

const NS = 'http://www.w3.org/2000/svg';

function createSearchIcon() {
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('width', '12');
  svg.setAttribute('height', '12');
  svg.setAttribute('fill', 'none');
  svg.setAttribute('stroke', 'currentColor');
  svg.setAttribute('stroke-width', '2');
  svg.setAttribute('stroke-linecap', 'round');
  svg.setAttribute('stroke-linejoin', 'round');
  svg.innerHTML = '<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>';
  return svg;
}

export function createInput({
  variant = 'default',
  placeholder = '',
  value = '',
  type = 'text',
} = {}) {
  const classes = ['ff-input'];
  if (variant === 'search') classes.push('ff-input--search');
  if (variant === 'numeric') classes.push('ff-input--numeric');

  const inputProps = {
    class: classes.join(' '),
    type,
    placeholder,
    value,
  };

  const input = el('input', inputProps);

  // Search variant: wrap with icon
  if (variant === 'search') {
    const iconWrap = el('span', { class: 'ff-input-wrap__icon' }, [createSearchIcon()]);
    const wrapper = el('div', { class: 'ff-input-wrap' }, [iconWrap, input]);
    return { element: wrapper };
  }

  return { element: input };
}
