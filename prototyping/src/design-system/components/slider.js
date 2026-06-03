import './slider.css';
import { el } from '../utils/dom.js';

/*
 * Slider primitive — horizontal range input.
 *   min/max: numeric bounds (default 0–100)
 *   value:   initial position (default 50)
 *   onChange: called with the new numeric value on drag
 */
export function createSlider({ min = 0, max = 100, value = 50, onChange } = {}) {
  let current = clamp(value, min, max);

  const fill = el('div', { class: 'ff-slider__fill' });
  const thumb = el('div', { class: 'ff-slider__thumb' });
  const track = el('div', { class: 'ff-slider__track' }, [fill, thumb]);
  const wrapper = el('div', { class: 'ff-slider' }, [track]);

  function clamp(v, lo, hi) {
    return Math.min(hi, Math.max(lo, v));
  }

  function ratio() {
    return max === min ? 0 : (current - min) / (max - min);
  }

  function render() {
    const pct = ratio() * 100;
    fill.style.width = `${pct}%`;
    thumb.style.left = `${pct}%`;
  }

  function setFromPointer(clientX) {
    const rect = track.getBoundingClientRect();
    const pct = clamp((clientX - rect.left) / rect.width, 0, 1);
    current = min + pct * (max - min);
    render();
    if (onChange) onChange(current);
  }

  function onPointerDown(e) {
    e.preventDefault();
    setFromPointer(e.clientX);
    wrapper.classList.add('ff-slider--dragging');

    function onMove(ev) {
      setFromPointer(ev.clientX);
    }
    function onUp() {
      wrapper.classList.remove('ff-slider--dragging');
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    }

    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
  }

  wrapper.addEventListener('mousedown', onPointerDown);

  render();

  return { element: wrapper };
}
