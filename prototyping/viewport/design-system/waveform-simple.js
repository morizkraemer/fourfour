/*
 * Row and detail-strip preview: one smooth loudness envelope, mirrored around the centre line,
 * one muted ink colour. No band layering, no transients. Scaling is absolute (the same input level
 * is the same height in every row), so loud tracks look loud and quiet ones look quiet. The area is
 * sampled at one column per device pixel at most, so it never aliases.
 */
import { ink } from './waveform-render.js';

export const LOUDNESS_GAIN = 1.5;
export const LOUDNESS_CURVE = 0.65;
export const MIN_HALF_FRACTION = 0.04;
const SMOOTHING_PASSES = 2;
const FILL_ALPHA = 0.58;

const envelopes = new WeakMap();

/** RMS over the three bands per column, scaled by the library gain, then smoothed over three columns twice. Memoised per bands object. */
export function loudnessEnvelope(bands) {
  let hit = envelopes.get(bands);
  if (hit) return hit;
  const n = bands.low.length;
  let values = new Float32Array(n);
  for (let i = 0; i < n; i++) {
    const l = bands.low[i];
    const m = bands.mid[i];
    const h = bands.high[i];
    values[i] = Math.min(1, Math.sqrt((l * l + m * m + h * h) / 2) * LOUDNESS_GAIN);
  }
  for (let pass = 0; pass < SMOOTHING_PASSES; pass++) {
    const next = new Float32Array(n);
    for (let i = 0; i < n; i++) next[i] = (values[Math.max(0, i - 1)] + values[i] * 2 + values[Math.min(n - 1, i + 1)]) / 4;
    values = next;
  }
  envelopes.set(bands, values);
  return values;
}

/** Display height (0..1) of one loudness level: gentle curve so quiet tracks stay readable, minimum silhouette. */
export function displayLevel(level) {
  return Math.max(MIN_HALF_FRACTION, Math.pow(Math.max(0, Math.min(1, level)), LOUDNESS_CURVE));
}

function sampleAt(values, position) {
  const n = values.length;
  const c = Math.max(0, Math.min(n - 1, position - 0.5));
  const i0 = Math.floor(c);
  const i1 = Math.min(n - 1, i0 + 1);
  return values[i0] + (values[i1] - values[i0]) * (c - i0);
}

/** Half height in device pixels for each of the widthPx output columns (one value per device pixel). */
export function halfHeights(values, widthPx, heightPx) {
  const yCentre = heightPx / 2;
  const out = new Float32Array(widthPx);
  for (let x = 0; x < widthPx; x++) out[x] = displayLevel(sampleAt(values, ((x + 0.5) / widthPx) * values.length)) * yCentre;
  return out;
}

/** Paint the preview of BandColumns into widthPx x heightPx device pixels (identity transform). */
export function drawLoudness(ctx, bands, { widthPx, heightPx, palette }) {
  if (!bands || !bands.low || !bands.low.length || widthPx <= 0) return;
  const halfs = halfHeights(loudnessEnvelope(bands), widthPx, heightPx);
  const yCentre = heightPx / 2;
  ctx.fillStyle = ink(palette, FILL_ALPHA);
  ctx.beginPath();
  ctx.moveTo(0, yCentre - halfs[0]);
  for (let x = 0; x < widthPx; x++) ctx.lineTo(x + 0.5, yCentre - halfs[x]);
  ctx.lineTo(widthPx, yCentre - halfs[widthPx - 1]);
  ctx.lineTo(widthPx, yCentre + halfs[widthPx - 1]);
  for (let x = widthPx - 1; x >= 0; x--) ctx.lineTo(x + 0.5, yCentre + halfs[x]);
  ctx.closePath();
  ctx.fill();
}
