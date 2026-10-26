/*
 * The one 3-band renderer. Draws BandColumns (see waveform-bands.js) into a 2D context
 * at device-pixel resolution: every output pixel column is one reduced column, so it is
 * crisp at any devicePixelRatio. Bands are symmetric around the centre line, drawn high
 * (back), mid, low (front). Short canvases fill one stepped path per band (cheap to
 * record, exact pixel edges); tall ones, which the zoom view repaints every frame, use one fillRect
 * per column and band because a jagged tall polygon is far slower to rasterise.
 */
import { OVERVIEW_COLUMNS, OVERVIEW_PEAK_WEIGHT, OVERVIEW_SCALE, halfHeight, reduceBands, smoothBands, stackHeights } from './waveform-bands.js';

const FALLBACK = {
  bg: '#0d0d0d',
  low: '#2f7bff',
  mid: '#ffa224',
  high: '#eef2f8',
  midInLow: '#b2660a',
  highInLow: '#ece8e5',
  highInMid: '#f6e4d2',
  dim: 'rgba(13, 13, 13, 0.58)',
  ink: '255, 255, 255',
  playhead: '#ffffff',
};

const TOKEN_NAMES = {
  bg: '--ff-wave-bg',
  low: '--ff-wave-low',
  mid: '--ff-wave-mid',
  high: '--ff-wave-high',
  midInLow: '--ff-wave-mid-in-low',
  highInLow: '--ff-wave-high-in-low',
  highInMid: '--ff-wave-high-in-mid',
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
 *
 * Rekordbox's stacking, back to front: low (blue), mid (amber) in front of it, high (white) in front of that,
 * every band from the centre line with its own height. Where a front band lies inside a back one it is drawn
 * in a darker tone (`midInLow`, `highInLow`, `highInMid`); only the part that sticks out keeps the bright tone.
 */
export function paintBands(ctx, shaped, { heightPx, palette, from = 0, count = shaped.length - from, offsetPx = 0 }) {
  const yCentre = heightPx / 2;
  if (heightPx <= STEPPED_PATH_MAX_HEIGHT) {
    paintStepped(ctx, shaped, palette, from, count, offsetPx, yCentre);
    return;
  }
  const layers = layerHalves(shaped, from, count, yCentre);
  for (let k = 0; k < STEPPED_LAYERS.length; k++) {
    const halves = layers[k];
    ctx.fillStyle = palette[STEPPED_LAYERS[k][0]];
    for (let p = 0; p < count; p++) {
      const half = halves[p];
      if (half > 0) fillColumn(ctx, offsetPx + p, yCentre, half);
    }
  }
}

/** Half height of every layer at every pixel column; 0 where a layer is not shown or a layer drawn after it covers it completely. */
function layerHalves(shaped, from, count, yCentre) {
  const halves = STEPPED_LAYERS.map(() => new Float32Array(count));
  for (let p = 0; p < count; p++) {
    const hl = halfHeight(shaped.low[from + p], 'low', yCentre);
    if (hl === 0) continue;
    const hm = halfHeight(shaped.mid[from + p], 'mid', yCentre);
    const hh = halfHeight(shaped.high[from + p], 'high', yCentre);
    for (let k = 0; k < STEPPED_LAYERS.length; k++) halves[k][p] = STEPPED_LAYERS[k][1](hl, hm, hh);
    for (let k = 0; k < STEPPED_LAYERS.length - 1; k++) {
      for (let j = k + 1; j < STEPPED_LAYERS.length; j++) {
        if (halves[j][p] >= halves[k][p]) {
          halves[k][p] = 0;
          break;
        }
      }
    }
  }
  return halves;
}

function fillColumn(ctx, x, yCentre, half) {
  const top = Math.round(yCentre - half);
  const bottom = Math.round(yCentre + half);
  ctx.fillRect(x, top, 1, Math.max(1, bottom - top));
}

const STEPPED_LAYERS = [
  ['low', (l) => l],
  ['mid', (l, m) => (m > l ? m : 0)],
  ['midInLow', (l, m) => Math.min(l, m)],
  ['high', (l, m, h) => (h > l ? h : 0)],
  ['highInLow', (l, m, h) => (Math.min(h, l) > Math.min(h, m) ? Math.min(h, l) : 0)],
  ['highInMid', (l, m, h) => Math.min(h, m)],
];

function paintStepped(ctx, shaped, palette, from, count, offsetPx, yCentre) {
  const halves = layerHalves(shaped, from, count, yCentre);
  for (let k = 0; k < STEPPED_LAYERS.length; k++) {
    paintSteppedLayer(ctx, halves[k], palette[STEPPED_LAYERS[k][0]], count, offsetPx, yCentre);
  }
}

function paintSteppedLayer(ctx, halves, colour, count, offsetPx, yCentre) {
  const tops = new Int16Array(count);
  const bottoms = new Int16Array(count);
  const centre = Math.round(yCentre);
  let any = false;
  for (let p = 0; p < count; p++) {
    const half = halves[p];
    if (!(half > 0)) {
      tops[p] = centre;
      bottoms[p] = centre;
      continue;
    }
    any = true;
    const top = Math.round(yCentre - half);
    tops[p] = top;
    bottoms[p] = Math.max(top + 1, Math.round(yCentre + half));
  }
  if (!any) return;
  ctx.fillStyle = colour;
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

/** Bottom-anchored stack (Rekordbox overview): high behind, then mid, then low in front, each from the baseline up to its cumulative height. */
export function paintStacked(ctx, shaped, { heightPx, palette, from = 0, count = shaped.length - from, offsetPx = 0 }) {
  const layers = [new Int16Array(count), new Int16Array(count), new Int16Array(count)];
  for (let p = 0; p < count; p++) {
    const stack = stackHeights(shaped, from + p, heightPx);
    for (let k = 0; k < 3; k++) layers[k][p] = stack ? Math.round(stack[k]) : 0;
  }
  const colours = [palette.low, palette.mid, palette.high];
  for (let k = 2; k >= 0; k--) paintBottomLayer(ctx, layers[k], colours[k], count, offsetPx, heightPx);
}

function paintBottomLayer(ctx, heights, colour, count, offsetPx, heightPx) {
  ctx.fillStyle = colour;
  ctx.beginPath();
  ctx.moveTo(offsetPx, heightPx);
  let last = heightPx;
  for (let p = 0; p < count; p++) {
    const y = heightPx - heights[p];
    if (y !== last) {
      ctx.lineTo(offsetPx + p, last);
      ctx.lineTo(offsetPx + p, y);
      last = y;
    }
  }
  ctx.lineTo(offsetPx + count, last);
  ctx.lineTo(offsetPx + count, heightPx);
  ctx.closePath();
  ctx.fill();
}

const SHAPED_PER_BANDS_LIMIT = 6;
const shapedCache = new WeakMap();

/** The reduced and smoothed columns of a static draw, kept per bands object and width: redraws (theme, dpr, bitmap eviction) skip the reduction. */
/** The overview at Rekordbox's resolution: at most OVERVIEW_COLUMNS mean buckets, each held over its share of the pixels. */
function reduceOverview(bands, startFrac, endFrac, widthPx) {
  const buckets = Math.min(widthPx, OVERVIEW_COLUMNS);
  const fromDetail = !bands.overview;
  const reduced = reduceBands(bands, startFrac, endFrac, buckets, OVERVIEW_PEAK_WEIGHT, fromDetail ? OVERVIEW_SCALE : null);
  if (buckets === widthPx) return reduced;
  const out = { low: new Float32Array(widthPx), mid: new Float32Array(widthPx), high: new Float32Array(widthPx), length: widthPx };
  for (let p = 0; p < widthPx; p++) {
    const k = Math.floor((p * buckets) / widthPx);
    out.low[p] = reduced.low[k];
    out.mid[p] = reduced.mid[k];
    out.high[p] = reduced.high[k];
  }
  return out;
}

function reduceStatic(bands, startFrac, endFrac, widthPx, decayPerPixel, stacked) {
  if (stacked) return reduceOverview(bands, startFrac, endFrac, widthPx);
  return smoothBands(reduceBands(bands, startFrac, endFrac, widthPx), decayPerPixel);
}

function shapedStatic(bands, startFrac, endFrac, widthPx, decayPerPixel, stacked) {
  if (!bands || typeof bands !== 'object') return reduceStatic(bands, startFrac, endFrac, widthPx, decayPerPixel, stacked);
  let byKey = shapedCache.get(bands);
  if (!byKey) {
    byKey = new Map();
    shapedCache.set(bands, byKey);
  }
  const key = `${startFrac}|${endFrac}|${widthPx}|${decayPerPixel}|${stacked}`;
  let shaped = byKey.get(key);
  if (!shaped) {
    shaped = reduceStatic(bands, startFrac, endFrac, widthPx, decayPerPixel, stacked);
    byKey.set(key, shaped);
    if (byKey.size > SHAPED_PER_BANDS_LIMIT) byKey.delete(byKey.keys().next().value);
  }
  return shaped;
}

/**
 * Reduce, smooth and paint the part [startFrac, endFrac] of the track into widthPx x heightPx device pixels.
 * `scrolling` aligns the sample grid to whole pixels (so peak buckets do not re-bucket while the viewport
 * slides) and warms the envelope up on a lead-in. The view moves in whole device pixels: a fractional paint
 * offset anti-aliases every one-pixel column against its neighbour, which lets the background bleed through
 * and makes the whole waveform pulse (up to 22 % brightness) as the sub-pixel phase changes.
 */
export function drawBandRange(ctx, bands, { startFrac, endFrac, widthPx, heightPx, palette, dpr = 1, decay = 0.88, scrolling = false, columnPx = 1, layout = 'centred' }) {
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
  const columnsPerPx = ((endFrac - startFrac) * (bands?.low?.length ?? 0)) / widthPx;
  const decayPerPixel = columnsPerPx <= 1 ? 0 : Math.pow(decay, 1 / dpr);
  if (!scrolling) {
    const stacked = layout === 'stacked';
    const shaped = shapedStatic(bands, startFrac, endFrac, widthPx, decayPerPixel, stacked);
    (stacked ? paintStacked : paintBands)(ctx, shaped, { heightPx, palette });
    return;
  }
  const fracPerPx = (endFrac - startFrac) / widthPx;
  if (columnsPerPx <= 1 && canCacheScroll(ctx)) {
    drawScrolledColumns(ctx, bands, { startFrac, fracPerPx, widthPx, heightPx, palette });
    return;
  }
  const leftFloor = Math.floor(startFrac / fracPerPx);
  const leadPx = Math.round(ENVELOPE_LEAD_CSS_PX * dpr);
  const total = leadPx + widthPx + 2;
  const shaped = smoothBands(
    reduceBands(bands, (leftFloor - leadPx) * fracPerPx, (leftFloor - leadPx + total) * fracPerPx, total),
    decayPerPixel,
  );
  paintBands(ctx, shaped, { heightPx, palette, from: leadPx, count: widthPx });
}

const scrollCache = new WeakMap();
const SCALE_TOLERANCE = 1e-9;

function sameScale(a, b) {
  return Math.abs(a - b) <= SCALE_TOLERANCE * Math.abs(b);
}

function canCacheScroll(ctx) {
  return typeof HTMLCanvasElement !== 'undefined' && ctx.canvas instanceof HTMLCanvasElement;
}

function newLayer(widthPx, heightPx) {
  const layer = document.createElement('canvas');
  layer.width = widthPx;
  layer.height = heightPx;
  return layer;
}

/**
 * Scrolled view at one pixel per column or finer: every pixel column depends only on its absolute pixel index (no
 * smoothing across columns), so a scroll by d pixels moves the bitmap of the last frame by d and paints only the d
 * columns that came into view. The bands of the whole view are painted again whenever the data, zoom, size or
 * palette change or the view jumped by more than its width.
 *
 * The view's width in track fractions comes from `endFrac - startFrac`, which differs in its last bits from frame to
 * frame as the view scrolls; the cache keeps the scale it was painted at and treats such a difference as the same
 * zoom (an exact comparison made about every other scrolled frame a full repaint).
 */
function drawScrolledColumns(ctx, bands, { startFrac, fracPerPx: viewFracPerPx, widthPx, heightPx, palette }) {
  const canvas = ctx.canvas;
  let entry = scrollCache.get(canvas);
  if (!entry || entry.widthPx !== widthPx || entry.heightPx !== heightPx) {
    entry = { widthPx, heightPx, front: newLayer(widthPx, heightPx), back: newLayer(widthPx, heightPx), left: null };
    scrollCache.set(canvas, entry);
  }
  if (entry.bands !== bands || !sameScale(entry.fracPerPx, viewFracPerPx) || entry.palette !== palette) {
    Object.assign(entry, { bands, fracPerPx: viewFracPerPx, palette, left: null });
  }
  const fracPerPx = entry.fracPerPx;
  const leftFloor = Math.floor(startFrac / fracPerPx);
  const paintColumns = (layer, fromPx, count) => {
    const g = layer.getContext('2d');
    g.clearRect(fromPx, 0, count, heightPx);
    const shaped = reduceBands(bands, (leftFloor + fromPx) * fracPerPx, (leftFloor + fromPx + count) * fracPerPx, count);
    paintBands(g, shaped, { heightPx, palette, offsetPx: fromPx });
  };
  const shift = entry.left === null ? widthPx : leftFloor - entry.left;
  if (Math.abs(shift) >= widthPx) {
    paintColumns(entry.front, 0, widthPx);
  } else if (shift !== 0) {
    const g = entry.back.getContext('2d');
    g.clearRect(0, 0, widthPx, heightPx);
    g.drawImage(entry.front, -shift, 0);
    [entry.front, entry.back] = [entry.back, entry.front];
    if (shift > 0) paintColumns(entry.front, widthPx - shift, shift);
    else paintColumns(entry.front, 0, -shift);
  }
  entry.left = leftFloor;
  ctx.drawImage(entry.front, 0, 0);
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
