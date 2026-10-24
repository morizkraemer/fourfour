/*
 * The one 3-band renderer. Draws BandColumns (see waveform-bands.js) into a 2D context
 * at device-pixel resolution: every output pixel column is one reduced column, so it is
 * crisp at any devicePixelRatio. Bands are symmetric around the centre line, drawn high
 * (back), mid, low (front). Short canvases fill one stepped path per band (cheap to
 * record, exact pixel edges); tall ones, which the zoom view repaints every frame, use one fillRect
 * per column and band because a jagged tall polygon is far slower to rasterise.
 */
import { BAND_GAIN, BAND_ORDER, halfHeight, reduceBands, smoothBands } from './waveform-bands.js';

const FALLBACK = {
  bg: '#0d0d0d',
  low: '#2f7bff',
  mid: '#ffa224',
  high: '#eef2f8',
  dim: 'rgba(13, 13, 13, 0.58)',
  ink: '255, 255, 255',
  playhead: '#ffffff',
};

const TOKEN_NAMES = {
  bg: '--ff-wave-bg',
  low: '--ff-wave-low',
  mid: '--ff-wave-mid',
  high: '--ff-wave-high',
  dim: '--ff-wave-dim',
  ink: '--ff-wave-ink',
  playhead: '--ff-wave-playhead',
};

const ENVELOPE_LEAD_CSS_PX = 64;
const STEPPED_PATH_MAX_HEIGHT = 100;

let cached = null;
let version = 0;

function invalidate() {
  cached = null;
  version++;
}

if (typeof document !== 'undefined') {
  new MutationObserver(invalidate).observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['class', 'style', 'data-theme'],
  });
  window.matchMedia?.('(prefers-color-scheme: dark)').addEventListener?.('change', invalidate);
}

/** Resolved waveform colours from the design tokens. Cached until the theme changes; never cached while a token is unresolved. */
export function wavePalette() {
  if (cached) return cached;
  if (typeof document === 'undefined') return { ...FALLBACK, version };
  const style = getComputedStyle(document.documentElement);
  const palette = { version };
  let complete = true;
  for (const [key, token] of Object.entries(TOKEN_NAMES)) {
    const value = style.getPropertyValue(token).trim();
    if (!value) complete = false;
    palette[key] = value || FALLBACK[key];
  }
  if (complete) cached = palette;
  return palette;
}

/** Changes whenever the theme tokens change; use in bitmap cache keys. */
export function paletteVersion() {
  return version;
}

/**
 * Fill the three band envelopes. `shaped` comes from reduceBands/smoothBands.
 * The context must have an identity transform (device pixels).
 */
export function paintBands(ctx, shaped, { heightPx, palette, from = 0, count = shaped.length - from, offsetPx = 0 }) {
  const yCentre = heightPx / 2;
  const stepped = heightPx <= STEPPED_PATH_MAX_HEIGHT;
  for (const band of BAND_ORDER) {
    const values = shaped[band];
    ctx.fillStyle = palette[band];
    if (stepped) {
      paintSteppedBand(ctx, values, band, from, count, offsetPx, yCentre);
      continue;
    }
    for (let p = 0; p < count; p++) {
      const half = halfHeight(values[from + p], band, yCentre);
      if (half === 0) continue;
      const top = Math.round(yCentre - half);
      const bottom = Math.round(yCentre + half);
      ctx.fillRect(offsetPx + p, top, 1, Math.max(1, bottom - top));
    }
  }
}

function paintSteppedBand(ctx, values, band, from, count, offsetPx, yCentre) {
  const gain = BAND_GAIN[band];
  const tops = new Int16Array(count);
  const bottoms = new Int16Array(count);
  const centre = Math.round(yCentre);
  for (let p = 0; p < count; p++) {
    const value = values[from + p];
    if (!(value > 0)) {
      tops[p] = centre;
      bottoms[p] = centre;
      continue;
    }
    const half = Math.max(0.5, Math.min(1, value * gain) * yCentre);
    const top = Math.round(yCentre - half);
    tops[p] = top;
    bottoms[p] = Math.max(top + 1, Math.round(yCentre + half));
  }
  ctx.beginPath();
  ctx.moveTo(offsetPx, centre);
  let last = centre;
  for (let p = 0; p < count; p++) {
    if (tops[p] !== last) {
      ctx.lineTo(offsetPx + p, last);
      ctx.lineTo(offsetPx + p, tops[p]);
      last = tops[p];
    }
  }
  ctx.lineTo(offsetPx + count, last);
  ctx.lineTo(offsetPx + count, centre);
  last = centre;
  for (let p = count - 1; p >= 0; p--) {
    if (bottoms[p] !== last) {
      ctx.lineTo(offsetPx + p + 1, last);
      ctx.lineTo(offsetPx + p + 1, bottoms[p]);
      last = bottoms[p];
    }
  }
  ctx.lineTo(offsetPx, last);
  ctx.closePath();
  ctx.fill();
}

const SHAPED_PER_BANDS_LIMIT = 6;
const shapedCache = new WeakMap();

/** The reduced and smoothed columns of a static draw, kept per bands object and width: redraws (theme, dpr, bitmap eviction) skip the reduction. */
function shapedStatic(bands, startFrac, endFrac, widthPx, decayPerPixel) {
  if (!bands || typeof bands !== 'object') return smoothBands(reduceBands(bands, startFrac, endFrac, widthPx), decayPerPixel);
  let byKey = shapedCache.get(bands);
  if (!byKey) {
    byKey = new Map();
    shapedCache.set(bands, byKey);
  }
  const key = `${startFrac}|${endFrac}|${widthPx}|${decayPerPixel}`;
  let shaped = byKey.get(key);
  if (!shaped) {
    shaped = smoothBands(reduceBands(bands, startFrac, endFrac, widthPx), decayPerPixel);
    byKey.set(key, shaped);
    if (byKey.size > SHAPED_PER_BANDS_LIMIT) byKey.delete(byKey.keys().next().value);
  }
  return shaped;
}

/**
 * Reduce, smooth and paint the part [startFrac, endFrac] of the track into widthPx x heightPx device pixels.
 * `scrolling` aligns the sample grid to whole pixels (so peak buckets do not re-bucket while the viewport
 * slides), warms the envelope up on a lead-in, and recovers the sub-pixel motion by translating the paint.
 */
export function drawBandRange(ctx, bands, { startFrac, endFrac, widthPx, heightPx, palette, dpr = 1, decay = 0.88, scrolling = false, columnPx = 1 }) {
  if (widthPx <= 0 || !(endFrac > startFrac)) return;
  if (columnPx > 1) {
    ctx.save();
    ctx.scale(columnPx, 1);
    drawBandRange(ctx, bands, {
      startFrac, endFrac, widthPx: Math.ceil(widthPx / columnPx), heightPx, palette, dpr: dpr / columnPx, decay, scrolling,
    });
    ctx.restore();
    return;
  }
  const decayPerPixel = Math.pow(decay, 1 / dpr);
  if (!scrolling) {
    paintBands(ctx, shapedStatic(bands, startFrac, endFrac, widthPx, decayPerPixel), { heightPx, palette });
    return;
  }
  const fracPerPx = (endFrac - startFrac) / widthPx;
  const leftReal = startFrac / fracPerPx;
  const leftFloor = Math.floor(leftReal);
  const subPx = leftReal - leftFloor;
  const leadPx = Math.round(ENVELOPE_LEAD_CSS_PX * dpr);
  const total = leadPx + widthPx + 2;
  const shaped = smoothBands(
    reduceBands(bands, (leftFloor - leadPx) * fracPerPx, (leftFloor - leadPx + total) * fracPerPx, total),
    decayPerPixel,
  );
  paintBands(ctx, shaped, { heightPx, palette, from: leadPx, count: widthPx + 2, offsetPx: -subPx });
}

/** Dim a played region [0, widthPx) on a canvas that has an opaque ground painted. */
export function dimPlayed(ctx, widthPx, heightPx, palette) {
  if (widthPx <= 0) return;
  ctx.fillStyle = palette.dim;
  ctx.fillRect(0, 0, widthPx, heightPx);
}

/** `rgba(...)` from the theme ink colour (grid lines, labels). */
export function ink(palette, alpha) {
  return `rgba(${palette.ink}, ${alpha})`;
}
