import './level-meter.css';
import { el } from '../utils/dom.js';

/*
 * Master level meter — a compact segmented stereo VU meter for the status bar.
 * Two stacked channel rows (L / R), each a strip of LED segments lit up to the
 * channel level. Segment color ramps green → warn → danger by position, so a
 * hot signal reads red at the top end. Tokens-only.
 *   left/right: 0..1 channel levels
 *   segments:   number of LEDs per channel
 *   label:      leading caption (set '' to hide)
 */
function channel(level, segments) {
  const lit = Math.round(Math.max(0, Math.min(1, level)) * segments);
  const blocks = [];
  for (let i = 0; i < segments; i++) {
    const frac = i / segments;
    let zone = 'green';
    if (frac >= 0.85) zone = 'danger';
    else if (frac >= 0.65) zone = 'warn';
    const on = i < lit ? ` ff-meter__seg--on ff-meter__seg--${zone}` : '';
    blocks.push(el('span', { class: `ff-meter__seg${on}` }));
  }
  return el('div', { class: 'ff-meter__channel' }, blocks);
}

export function createLevelMeter({ left = 0.72, right = 0.64, segments = 16, label = 'MASTER' } = {}) {
  const children = [];
  if (label) children.push(el('span', { class: 'ff-meter__label', text: label }));
  children.push(
    el('div', { class: 'ff-meter__channels' }, [
      channel(left, segments),
      channel(right, segments),
    ])
  );
  return { element: el('div', { class: 'ff-meter' }, children) };
}
