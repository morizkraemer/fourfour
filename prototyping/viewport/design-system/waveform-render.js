/*
 * The one 3-band renderer. Draws BandColumns (see waveform-bands.js) into a 2D context
 * at device-pixel resolution: every output pixel column is one reduced column, so it is
 * crisp at any devicePixelRatio. Bands are symmetric around the centre line, drawn high
 * (back), mid, low (front). Short canvases fill one stepped path per band (cheap to
 * record, exact pixel edges); tall ones, which the zoom view repaints every frame, use one fillRect
 * per column and band because a jagged tall polygon is far slower to rasterise.
 */
import { paintZoom } from './waveform-zoom.js';
import { BAND_GAIN, BAND_ORDER, BUCKET_PEAK_WEIGHT, OVERVIEW_COLUMNS, OVERVIEW_PEAK_WEIGHT, OVERVIEW_SCALE, OVERVIEW_SMOOTH_COLUMNS, halfHeight, reduceBands, smoothBands, smoothTriangle, stackHeights } from './waveform-bands.js';

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
  gridBar: 0.7,
  gridBeat: 0.34,
  gridEighth: 0.18,
  gridSixteenth: 0.1,
};

const NUMBER_TOKENS = new Set(['gridBar', 'gridBeat', 'gridEighth', 'gridSixteenth']);

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
  gridBar: '--ff-wave-grid-bar-alpha',
  gridBeat: '--ff-wave-grid-beat-alpha',
  gridEighth: '--ff-wave-grid-eighth-alpha',
  gridSixteenth: '--ff-wave-grid-sixteenth-alpha',
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
    palette[key] = !value ? FALLBACK[key] : NUMBER_TOKENS.has(key) ? Number(value) : value;
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
export function paintBands(ctx, shaped, { heightPx, palette, from = 0, count = shaped.length - from, offsetPx = 0, shapeMix = 0, minSpanPx = 1 }) {
  const yCentre = heightPx / 2;
  if (shapeMix > 0 && shaped.top) {
    paintSignalShaped(ctx, shaped, { palette, from, count, offsetPx, yCentre, shapeMix, minSpanPx });
    return;
  }
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
export function layerHalves(shaped, from, count, yCentre) {
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

/**
 * Deep zoom: the silhouette is the audio itself. Each pixel column spans from its lowest to its highest sample (`bottom`,
 * `top`), moved `shapeMix` of the way there from the symmetric band envelope, so zooming in morphs the band look into the
 * signal without a jump. A column reaches over to the previous one's extremes, so a curve finer than a sample per pixel stays
 * one connected trace at least `minSpanPx` thick. The bands keep their colours and nesting inside that span: each layer gets
 * the share of the span its envelope has of the outermost one.
 */
function paintSignalShaped(ctx, shaped, { palette, from, count, offsetPx, yCentre, shapeMix, minSpanPx }) {
  const halves = layerHalves(shaped, from, count, yCentre);
  const outerAt = (i) => Math.max(
    halfHeight(shaped.low[i], 'low', yCentre),
    halfHeight(shaped.mid[i], 'mid', yCentre),
    halfHeight(shaped.high[i], 'high', yCentre),
  );
  const signalAt = (v) => Math.max(-1, Math.min(1, v * BAND_GAIN.low)) * yCentre;
  const spanAt = (i) => {
    const outer = outerAt(i);
    return [
      outer + (signalAt(shaped.top[i]) - outer) * shapeMix,
      -outer + (signalAt(shaped.bottom[i]) + outer) * shapeMix,
      outer,
    ];
  };
  let prev = from > 0 && shaped.low[from - 1] > 0 ? spanAt(from - 1) : null;
  const tops = new Float32Array(count);
  const bottoms = new Float32Array(count);
  const outers = new Float32Array(count);
  for (let p = 0; p < count; p++) {
    const i = from + p;
    if (!(shaped.low[i] > 0)) {
      prev = null;
      continue;
    }
    const span = spanAt(i);
    let [top, bottom] = span;
    if (prev) {
      top = Math.max(top, prev[1]);
      bottom = Math.min(bottom, prev[0]);
    }
    if (top - bottom < minSpanPx) {
      const centre = (top + bottom) / 2;
      top = centre + minSpanPx / 2;
      bottom = centre - minSpanPx / 2;
    }
    prev = span;
    tops[p] = top;
    bottoms[p] = bottom;
    outers[p] = span[2];
  }
  for (let k = 0; k < STEPPED_LAYERS.length; k++) {
    const layer = halves[k];
    ctx.fillStyle = palette[STEPPED_LAYERS[k][0]];
    for (let p = 0; p < count; p++) {
      const half = layer[p];
      if (!(half > 0) || !(outers[p] > 0)) continue;
      const centre = (tops[p] + bottoms[p]) / 2;
      const reach = (half / outers[p]) * (tops[p] - bottoms[p]) / 2;
      if (reach * 2 < 0.5) continue;
      const y0 = Math.round(yCentre - centre - reach);
      const y1 = Math.round(yCentre - centre + reach);
      ctx.fillRect(offsetPx + p, y0, 1, Math.max(1, y1 - y0));
    }
  }
}

function fillColumn(ctx, x, yCentre, half) {
  const top = Math.round(yCentre - half);
  const bottom = Math.round(yCentre + half);
  ctx.fillRect(x, top, 1, Math.max(1, bottom - top));
}

export const STEPPED_LAYERS = [
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
/**
 * The overview at about Rekordbox's resolution: one mean per pixel, then a triangle filter as wide as
 * OVERVIEW_SMOOTH_COLUMNS of Rekordbox's OVERVIEW_COLUMNS. Holding 1200 buckets over a wider canvas instead
 * gave columns of one and two pixels in turn, a comb of spikes along the top edge.
 */
function reduceOverview(bands, startFrac, endFrac, widthPx) {
  const fromDetail = !bands.overview;
  const reduced = reduceBands(bands, startFrac, endFrac, widthPx, OVERVIEW_PEAK_WEIGHT, fromDetail ? OVERVIEW_SCALE : null);
  const radiusPx = Math.max(1, (OVERVIEW_SMOOTH_COLUMNS * widthPx) / OVERVIEW_COLUMNS);
  for (const name of BAND_ORDER) reduced[name] = smoothTriangle(reduced[name], radiusPx);
  return reduced;
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
 * `ground` (a colour) makes the call cover the whole width opaquely: the scrolled view's cached layers hold it, every
 * other path fills it first, so the caller needs no separate fill of the same area.
 */
export function drawBandRange(ctx, bands, { startFrac, endFrac, widthPx, heightPx, palette, dpr = 1, decay = 0.88, scrolling = false, columnPx = 1, layout = 'centred', peakWeights = BUCKET_PEAK_WEIGHT, zoomLook = null, msPerPx = 0, ground = null }) {
  if (widthPx <= 0 || !(endFrac > startFrac)) return;
  if (columnPx > 1) {
    ctx.save();
    ctx.scale(columnPx, 1);
    drawBandRange(ctx, bands, {
      startFrac, endFrac, widthPx: Math.ceil(widthPx / columnPx), heightPx, palette, dpr: dpr / columnPx, decay, scrolling, peakWeights,
    });
    ctx.restore();
    return;
  }
  const columnsPerPx = ((endFrac - startFrac) * (bands?.low?.length ?? 0)) / widthPx;
  const decayPerPixel = columnsPerPx <= 1 ? 0 : Math.pow(decay, 1 / dpr);
  const fillGround = () => {
    if (!ground) return;
    ctx.fillStyle = ground;
    ctx.fillRect(0, 0, widthPx, heightPx);
  };
  if (!scrolling) {
    fillGround();
    const stacked = layout === 'stacked';
    const shaped = shapedStatic(bands, startFrac, endFrac, widthPx, decayPerPixel, stacked);
    (stacked ? paintStacked : paintBands)(ctx, shaped, { heightPx, palette });
    return;
  }
  const fracPerPx = (endFrac - startFrac) / widthPx;
  const paint = (g, shaped, at) => (zoomLook
    ? paintZoom(g, shaped, { look: zoomLook, heightPx, palette, msPerPx, dpr, ...at })
    : paintBands(g, shaped, { heightPx, palette, ...at }));
  if (decayPerPixel === 0 && canCacheScroll(ctx)) {
    drawScrolledColumns(ctx, bands, { startFrac, fracPerPx, widthPx, heightPx, peakWeights, paint, ground, key: `${zoomLook}|${palette.version}|${palette.bg}|${ground}` });
    return;
  }
  fillGround();
  const leftFloor = Math.floor(startFrac / fracPerPx);
  const leadPx = Math.round(ENVELOPE_LEAD_CSS_PX * dpr);
  const total = leadPx + widthPx + 2;
  const shaped = smoothBands(
    reduceBands(bands, (leftFloor - leadPx) * fracPerPx, (leftFloor - leadPx + total) * fracPerPx, total, peakWeights),
    decayPerPixel,
  );
  paint(ctx, shaped, { from: leadPx, count: widthPx });
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
 * Scrolled view without an envelope decay (one pixel per column or finer, or the high-resolution series at any zoom):
 * every pixel column depends only on its absolute pixel index (no smoothing across columns), so a scroll by d pixels
 * paints only the d columns that came into view. The columns live
 * in a ring: absolute pixel column c sits at layer x = c mod width, so a scroll overwrites the columns that left the view
 * and nothing is copied; the layer is blitted to the view in two slices at the wrap point. The bands of the whole view are
 * painted again whenever the data, zoom, size or palette change or the view jumped by more than its width.
 *
 * The view's width in track fractions comes from `endFrac - startFrac`, which differs in its last bits from frame to
 * frame as the view scrolls; the cache keeps the scale it was painted at and treats such a difference as the same
 * zoom (an exact comparison made about every other scrolled frame a full repaint).
 */
function drawScrolledColumns(ctx, bands, { startFrac, fracPerPx: viewFracPerPx, widthPx, heightPx, peakWeights, paint, ground, key }) {
  const canvas = ctx.canvas;
  let entry = scrollCache.get(canvas);
  if (!entry || entry.widthPx !== widthPx || entry.heightPx !== heightPx) {
    entry = { widthPx, heightPx, layer: newLayer(widthPx, heightPx), left: null };
    scrollCache.set(canvas, entry);
  }
  if (entry.bands !== bands || !sameScale(entry.fracPerPx, viewFracPerPx) || entry.key !== key) {
    Object.assign(entry, { bands, fracPerPx: viewFracPerPx, key, left: null });
  }
  const fracPerPx = entry.fracPerPx;
  const leftFloor = Math.floor(startFrac / fracPerPx);
  const ringX = (absolute) => ((absolute % widthPx) + widthPx) % widthPx;
  const paintColumns = (absoluteFrom, count) => {
    const g = entry.layer.getContext('2d');
    for (let done = 0; done < count;) {
      const first = absoluteFrom + done;
      const x = ringX(first);
      const run = Math.min(count - done, widthPx - x);
      if (ground) {
        g.fillStyle = ground;
        g.fillRect(x, 0, run, heightPx);
      } else {
        g.clearRect(x, 0, run, heightPx);
      }
      const shaped = reduceBands(bands, (first - 1) * fracPerPx, (first + run) * fracPerPx, run + 1, peakWeights);
      paint(g, shaped, { from: 1, count: run, offsetPx: x });
      done += run;
    }
  };
  const shift = entry.left === null ? widthPx : leftFloor - entry.left;
  if (Math.abs(shift) >= widthPx) paintColumns(leftFloor, widthPx);
  else if (shift > 0) paintColumns(entry.left + widthPx, shift);
  else if (shift < 0) paintColumns(leftFloor, -shift);
  entry.left = leftFloor;
  const split = ringX(leftFloor);
  if (split === 0) {
    ctx.drawImage(entry.layer, 0, 0);
    return;
  }
  ctx.drawImage(entry.layer, split, 0, widthPx - split, heightPx, 0, 0, widthPx - split, heightPx);
  ctx.drawImage(entry.layer, 0, 0, split, heightPx, widthPx - split, 0, split, heightPx);
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
