/*
 * Dev switch for the static row and detail previews: `?wave=a|e|f|g`. No param: `a`, the
 * unchanged 3-band baseline. The simplified modes are drawn by waveform-simple.js.
 */

export const WAVE_VARIANTS = {
  a: { label: 'Current 3-band' },
  e: { label: 'Smooth loudness area' },
  f: { label: 'Chunky energy bars' },
  g: { label: 'Global gain + breakdown dips' },
};

export const WAVE_VARIANT_KEYS = Object.keys(WAVE_VARIANTS);

const listeners = new Set();
const urlParam = typeof location === 'undefined' ? null : new URLSearchParams(location.search).get('wave');
let current = urlParam !== null && WAVE_VARIANTS[urlParam] ? urlParam : 'a';

/** The dev switch exists only when the URL carries `?wave=`. */
export function waveSwitchActive() {
  return urlParam !== null;
}

export function currentWaveVariant() {
  return current;
}

export function setWaveVariant(key) {
  if (!WAVE_VARIANTS[key] || key === current) return;
  current = key;
  for (const listener of listeners) listener(key);
}

export function onWaveVariant(listener) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
