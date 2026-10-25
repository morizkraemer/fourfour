/*
 * Display scaling presets for the static row and detail waveforms, switched with `?wave=a|b|c|d`.
 * No param: `a`, the unchanged baseline. A spec is passed into reduceBands / paintBands; the
 * per-track normalisation runs once per bands object (WeakMap) before the renderer sees it.
 */
import { BAND_GAIN, DISPLAY_FLOOR, displayAmp } from './waveform-bands.js';

const HISTOGRAM_BINS = 1024;

const logCurve = (strength) => (a) => Math.log1p(strength * a) / Math.log1p(strength);
const powerCurve = (gamma) => (a) => Math.pow(a, gamma);

export const WAVE_SCALINGS = {
  a: {
    name: 'a',
    label: 'Current',
    curve: powerCurve(0.65),
    floor: DISPLAY_FLOOR,
    silenceFloor: DISPLAY_FLOOR,
    gains: BAND_GAIN,
    normalize: null,
    smooth: false,
  },
  b: {
    name: 'b',
    label: 'Auto-gain',
    curve: powerCurve(0.65),
    floor: DISPLAY_FLOOR,
    silenceFloor: DISPLAY_FLOOR,
    gains: BAND_GAIN,
    normalize: { percentile: 0.95, target: 0.92, minGain: 0.7, maxGain: 6 },
    smooth: false,
  },
  c: {
    name: 'c',
    label: 'Auto-gain + log curve',
    curve: logCurve(7),
    floor: 0.05,
    silenceFloor: 0.03,
    gains: { low: 0.66, mid: 0.9, high: 1 },
    normalize: { percentile: 0.95, target: 0.7, minGain: 0.7, maxGain: 6 },
    smooth: false,
  },
  d: {
    name: 'd',
    label: 'Rekordbox-style',
    curve: powerCurve(0.5),
    floor: 0.14,
    silenceFloor: 0.02,
    gains: { low: 0.55, mid: 0.85, high: 0.8 },
    normalize: { percentile: 0.95, target: 0.82, minGain: 0.7, maxGain: 6 },
    smooth: true,
  },
};

export const WAVE_VARIANT_KEYS = Object.keys(WAVE_SCALINGS);

export function scalingFor(key) {
  return WAVE_SCALINGS[key] ?? WAVE_SCALINGS.a;
}

/** Raw 0..1 amplitude to display height under a spec. Exact zero is silence, anything else is music with a minimum silhouette. */
export function scaledAmp(raw, spec) {
  if (!spec || spec === WAVE_SCALINGS.a) return displayAmp(raw);
  const a = Math.max(0, Math.min(1, raw));
  if (a <= 0.004) return spec.silenceFloor;
  return spec.floor + (1 - spec.floor) * spec.curve(a);
}

function percentileOf(values, p) {
  const bins = new Uint32Array(HISTOGRAM_BINS);
  for (let i = 0; i < values.length; i++) bins[Math.min(HISTOGRAM_BINS - 1, Math.floor(values[i] * HISTOGRAM_BINS))]++;
  const target = values.length * p;
  let seen = 0;
  for (let b = 0; b < HISTOGRAM_BINS; b++) {
    seen += bins[b];
    if (seen >= target) return (b + 1) / HISTOGRAM_BINS;
  }
  return 1;
}

function smoothed(src) {
  const out = new Float32Array(src.length);
  for (let i = 0; i < src.length; i++) {
    const prev = src[Math.max(0, i - 1)];
    const next = src[Math.min(src.length - 1, i + 1)];
    out[i] = Math.max(src[i] * 0.7 + (prev + next) * 0.15, src[i] * 0.85);
  }
  return out;
}

const prepared = new WeakMap();

/**
 * Per-track normalisation: one gain for all three bands (ratios kept), chosen so the 95th percentile
 * of the drawn envelope reaches `target`. Returns the same object when the spec asks for nothing.
 */
export function prepareBands(bands, spec) {
  if (!bands || !bands.low || (!spec.normalize && !spec.smooth)) return bands;
  let bySpec = prepared.get(bands);
  if (!bySpec) {
    bySpec = new Map();
    prepared.set(bands, bySpec);
  }
  let out = bySpec.get(spec.name);
  if (out) return out;
  const n = bands.low.length;
  let gain = 1;
  if (spec.normalize && n) {
    const envelope = new Float32Array(n);
    for (let i = 0; i < n; i++) {
      envelope[i] = Math.min(1, Math.max(bands.low[i] * spec.gains.low, bands.mid[i] * spec.gains.mid, bands.high[i] * spec.gains.high));
    }
    const p = percentileOf(envelope, spec.normalize.percentile);
    gain = p > 0.01 ? Math.min(spec.normalize.maxGain, Math.max(spec.normalize.minGain, spec.normalize.target / p)) : 1;
  }
  out = {};
  for (const name of ['low', 'mid', 'high']) {
    const src = spec.smooth ? smoothed(bands[name]) : bands[name];
    const dst = new Float32Array(n);
    for (let i = 0; i < n; i++) dst[i] = Math.min(1, src[i] * gain);
    out[name] = dst;
  }
  bySpec.set(spec.name, out);
  return out;
}

const listeners = new Set();
const urlParam = typeof location === 'undefined' ? null : new URLSearchParams(location.search).get('wave');
let current = urlParam !== null && WAVE_SCALINGS[urlParam] ? urlParam : 'a';

/** The dev switch exists only when the URL carries `?wave=`. */
export function waveSwitchActive() {
  return urlParam !== null;
}

export function currentWaveVariant() {
  return current;
}

export function setWaveVariant(key) {
  if (!WAVE_SCALINGS[key] || key === current) return;
  current = key;
  for (const listener of listeners) listener(key);
}

export function onWaveVariant(listener) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
