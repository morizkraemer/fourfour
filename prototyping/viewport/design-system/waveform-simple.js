/*
 * Simplified row and detail previews: one loudness envelope, no band layering, no transients.
 * All modes use absolute scaling (the same input level is the same height in every row), so loud
 * tracks look loud and quiet ones look quiet. Columns are drawn at most one per device pixel.
 *
 *   e  smooth mirrored area of the loudness envelope
 *   f  chunky mirrored bars (RMS per span), colour ramps blue to amber with energy
 *   g  like e with a fixed library gain, breakdowns pushed down to a hairline, faint baseline
 */
import { ink } from './waveform-render.js';

const LOUDNESS_GAIN = 1.5;
const LIBRARY_REFERENCE_LEVEL = 0.95;
const BREAKDOWN_RATIO = 0.4;
const BREAKDOWN_DEPTH = 0.25;
const MIN_HALF_FRACTION = 0.04;
const BARS = { mini: { pitchPx: 2.5, gapPx: 1 }, strip: { pitchPx: 5, gapPx: 1.5 } };

const envelopes = new WeakMap();

/** Loudness per source column: RMS over the three bands, smoothed over three columns twice. Memoised per bands object. */
function loudnessOf(bands) {
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
  for (let pass = 0; pass < 2; pass++) {
    const next = new Float32Array(n);
    for (let i = 0; i < n; i++) next[i] = (values[Math.max(0, i - 1)] + values[i] * 2 + values[Math.min(n - 1, i + 1)]) / 4;
    values = next;
  }
  const sorted = Float32Array.from(values).sort();
  hit = { values, median: sorted[n >> 1] };
  envelopes.set(bands, hit);
  return hit;
}

function sampleAt(values, position) {
  const n = values.length;
  const c = Math.max(0, Math.min(n - 1, position - 0.5));
  const i0 = Math.floor(c);
  const i1 = Math.min(n - 1, i0 + 1);
  return values[i0] + (values[i1] - values[i0]) * (c - i0);
}

function smoothstep(edge0, edge1, x) {
  const t = Math.max(0, Math.min(1, (x - edge0) / (edge1 - edge0)));
  return t * t * (3 - 2 * t);
}

function areaHeight(mode, level, median) {
  if (mode === 'g') {
    let v = Math.min(1, level / LIBRARY_REFERENCE_LEVEL);
    const breakdown = 1 - smoothstep(BREAKDOWN_RATIO * median * 0.9, BREAKDOWN_RATIO * median * 1.1, level);
    v *= 1 - breakdown * (1 - BREAKDOWN_DEPTH);
    return v;
  }
  return Math.pow(level, 0.65);
}

function paintArea(ctx, data, mode, widthPx, heightPx, palette) {
  const yCentre = heightPx / 2;
  const n = data.values.length;
  const halfs = new Float32Array(widthPx);
  for (let x = 0; x < widthPx; x++) {
    const level = sampleAt(data.values, ((x + 0.5) / widthPx) * n);
    halfs[x] = Math.max(MIN_HALF_FRACTION, areaHeight(mode, level, data.median)) * yCentre;
  }
  if (mode === 'g') {
    ctx.fillStyle = ink(palette, 0.16);
    ctx.fillRect(0, Math.round(yCentre - 0.5), widthPx, Math.max(1, Math.round(heightPx / 28)));
  }
  ctx.fillStyle = ink(palette, mode === 'g' ? 0.72 : 0.58);
  ctx.beginPath();
  ctx.moveTo(0, yCentre - halfs[0]);
  for (let x = 0; x < widthPx; x++) ctx.lineTo(x + 0.5, yCentre - halfs[x]);
  ctx.lineTo(widthPx, yCentre - halfs[widthPx - 1]);
  ctx.lineTo(widthPx, yCentre + halfs[widthPx - 1]);
  for (let x = widthPx - 1; x >= 0; x--) ctx.lineTo(x + 0.5, yCentre + halfs[x]);
  ctx.closePath();
  ctx.fill();
}

function paintBars(ctx, data, variant, widthPx, heightPx, dpr, palette) {
  const { pitchPx, gapPx } = BARS[variant] ?? BARS.mini;
  const count = Math.max(1, Math.round(widthPx / (pitchPx * dpr)));
  const n = data.values.length;
  const yCentre = heightPx / 2;
  const gap = Math.max(1, Math.round(gapPx * dpr));
  for (let b = 0; b < count; b++) {
    const from = Math.floor((b * n) / count);
    const to = Math.max(from + 1, Math.floor(((b + 1) * n) / count));
    let sum = 0;
    for (let i = from; i < to; i++) sum += data.values[i] * data.values[i];
    const level = Math.sqrt(sum / (to - from));
    const half = Math.max(MIN_HALF_FRACTION, Math.pow(level, 0.65)) * yCentre;
    const x0 = Math.round((b * widthPx) / count);
    const x1 = Math.round(((b + 1) * widthPx) / count) - gap;
    const top = Math.round(yCentre - half);
    const height = Math.max(1, Math.round(yCentre + half) - top);
    ctx.fillStyle = palette.low;
    ctx.fillRect(x0, top, Math.max(1, x1 - x0), height);
    ctx.globalAlpha = Math.min(1, level / 0.7);
    ctx.fillStyle = palette.mid;
    ctx.fillRect(x0, top, Math.max(1, x1 - x0), height);
    ctx.globalAlpha = 1;
  }
}

/** Paint a simplified preview of BandColumns into widthPx x heightPx device pixels (identity transform). */
export function drawSimple(ctx, bands, { mode, variant, widthPx, heightPx, dpr, palette }) {
  if (!bands || !bands.low || !bands.low.length || widthPx <= 0) return;
  const data = loudnessOf(bands);
  if (mode === 'f') paintBars(ctx, data, variant, widthPx, heightPx, dpr, palette);
  else paintArea(ctx, data, mode, widthPx, heightPx, palette);
}
