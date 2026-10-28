/*
 * How the zoom view turns from the 3-band envelope into the audio itself as it closes in. One painter per look, all
 * taking the reduced columns of reduceBands (band heights plus the lowest and highest sample, `bottom` / `top`) at one
 * device pixel per column. `ZOOM_LOOK` picks the one the app draws; the mockup page (`prototyping/mockups`, zoom-look)
 * shows them side by side.
 *
 *   morph    the envelope's span slides towards each column's sample span (the look of round 10)
 *   body     symmetric 3-band body at every zoom; its outline sharpens to the column's loudest sample
 *   signal   envelope while zoomed out, then the sample span filled in one colour mixed from the bands, a line below a sample per pixel
 *   layered  envelope stays as a soft fill under the sample span drawn as an outline
 *   filled   the signal filled to the centre line, the bands nested inside it from the centre out
 */
import { BAND_GAIN } from './waveform-bands.js';
import { STEPPED_LAYERS, layerHalves, paintBands } from './waveform-render.js';

export const ZOOM_LOOKS = ['morph', 'body', 'signal', 'layered', 'filled'];
export const ZOOM_LOOK = 'filled';

/** Time per device pixel where the signal starts to show, and where it has taken over. */
export const SHAPE_FROM_MS_PER_PX = 2;
export const SHAPE_FULL_MS_PER_PX = 0.5;

const LAYERED_BAND_ALPHA = 0.38;
const SIGNAL_MIN_THICKNESS_CSS_PX = 1.25;
const DOTS_FROM_CSS_PX_PER_SAMPLE = 5;
const DOT_RADIUS_PER_SAMPLE_PX = 0.09;
const DOT_MAX_RADIUS_CSS_PX = 2.5;

/** 0 (envelope) to 1 (signal) from the time one device pixel covers, eased, continuous in the zoom. */
export function shapeAmount(msPerPx, from = SHAPE_FROM_MS_PER_PX, full = SHAPE_FULL_MS_PER_PX) {
  if (!(msPerPx > 0)) return 0;
  const t = Math.max(0, Math.min(1, Math.log(from / msPerPx) / Math.log(from / full)));
  return t * t * (3 - 2 * t);
}

/** The linear blend `paintBands` used for its morph (round 10), kept for the `morph` look. */
function morphAmount(msPerPx) {
  if (!(msPerPx > 0)) return 0;
  return Math.max(0, Math.min(1, Math.log(SHAPE_FROM_MS_PER_PX / msPerPx) / Math.log(SHAPE_FROM_MS_PER_PX / 0.4)));
}

/**
 * Paint `count` reduced columns from `from` at device x `offsetPx` in `look`. `msPerPx` is the time one device pixel
 * covers; `dpr` sizes the thinnest line.
 */
export function paintZoom(ctx, cols, { look = ZOOM_LOOK, heightPx, palette, msPerPx, dpr = 1, from = 0, count = cols.length - from, offsetPx = 0 }) {
  const hasSignal = !!(cols.top && cols.bottom);
  if (look === 'morph' || !hasSignal) {
    const shapeMix = hasSignal && look === 'morph' ? morphAmount(msPerPx) : 0;
    paintBands(ctx, cols, { heightPx, palette, from, count, offsetPx, shapeMix, minSpanPx: Math.max(1, dpr) });
    return;
  }
  if (count <= 0 || heightPx <= 0) return;
  const pixels = PixelColumns.for(ctx, offsetPx, count, heightPx, palette);
  paintLook(pixels, cols, { look, heightPx, palette, msPerPx, dpr, from, count, offsetPx });
  pixels.flushTo(ctx);
}

function paintLook(ctx, cols, { look, heightPx, palette, msPerPx, dpr, from, count, offsetPx }) {
  const yCentre = heightPx / 2;
  const s = shapeAmount(msPerPx);
  const halves = layerHalves(cols, from, count, yCentre);
  const outer = outerHalves(halves, count);
  const signalPx = (v) => Math.max(-1, Math.min(1, v * BAND_GAIN.low)) * yCentre;
  const minThickness = Math.max(1, Math.round(SIGNAL_MIN_THICKNESS_CSS_PX * dpr));

  if (look === 'body') {
    const scale = new Float32Array(count);
    for (let p = 0; p < count; p++) {
      if (!(outer[p] > 0)) continue;
      const peak = Math.max(Math.abs(signalPx(cols.top[from + p])), Math.abs(signalPx(cols.bottom[from + p])));
      scale[p] = 1 + (Math.max(peak, minThickness / 2) / outer[p] - 1) * s;
    }
    paintLayers(ctx, halves, palette, { count, offsetPx, yCentre, up: scale, down: scale });
    return;
  }

  if (look === 'filled') {
    const up = new Float32Array(count);
    const down = new Float32Array(count);
    for (let p = 0; p < count; p++) {
      if (!(outer[p] > 0)) continue;
      const top = Math.max(0, signalPx(cols.top[from + p]));
      const bottom = Math.max(0, -signalPx(cols.bottom[from + p]));
      up[p] = 1 + (top / outer[p] - 1) * s;
      down[p] = 1 + (bottom / outer[p] - 1) * s;
    }
    paintLayers(ctx, halves, palette, { count, offsetPx, yCentre, up, down, minPx: s > 0.5 ? 0 : 1 });
    return;
  }

  if (look === 'layered') {
    ctx.save();
    ctx.globalAlpha = 1 + (LAYERED_BAND_ALPHA - 1) * s;
    paintLayers(ctx, halves, palette, { count, offsetPx, yCentre });
    ctx.restore();
    if (s <= 0) return;
    ctx.save();
    ctx.globalAlpha = s;
    ctx.fillStyle = palette.high;
    paintSpanOutline(ctx, cols, { from, count, offsetPx, yCentre, signalPx, thickness: minThickness, outer });
    ctx.restore();
    return;
  }

  if (look === 'signal') {
    if (s < 1) {
      ctx.save();
      ctx.globalAlpha = 1 - s;
      paintLayers(ctx, halves, palette, { count, offsetPx, yCentre });
      ctx.restore();
    }
    if (s <= 0) return;
    ctx.save();
    ctx.globalAlpha = s;
    const mix = bandMixer(palette);
    for (let p = 0; p < count; p++) {
      const i = from + p;
      if (!(outer[p] > 0)) continue;
      let top = yCentre - signalPx(cols.top[i]);
      let bottom = yCentre - signalPx(cols.bottom[i]);
      if (bottom - top < minThickness) {
        const centre = (top + bottom) / 2;
        top = centre - minThickness / 2;
        bottom = centre + minThickness / 2;
      }
      ctx.fillStyle = mix(cols.low[i], cols.mid[i], cols.high[i]);
      const y0 = Math.round(top);
      ctx.fillRect(offsetPx + p, y0, 1, Math.max(1, Math.round(bottom) - y0));
    }
    ctx.restore();
  }
}

function outerHalves(halves, count) {
  const outer = new Float32Array(count);
  for (const layer of halves) {
    for (let p = 0; p < count; p++) if (layer[p] > outer[p]) outer[p] = layer[p];
  }
  return outer;
}

/** The band layers, each column scaled by `up` above the centre line and `down` below it (1 when not given). */
function paintLayers(ctx, halves, palette, { count, offsetPx, yCentre, up = null, down = null, minPx = 1 }) {
  for (let k = 0; k < STEPPED_LAYERS.length; k++) {
    const layer = halves[k];
    ctx.fillStyle = palette[STEPPED_LAYERS[k][0]];
    for (let p = 0; p < count; p++) {
      const half = layer[p];
      if (!(half > 0)) continue;
      const y0 = Math.round(yCentre - half * (up ? up[p] : 1));
      const y1 = Math.round(yCentre + half * (down ? down[p] : 1));
      if (y1 - y0 < minPx) {
        if (minPx > 0) ctx.fillRect(offsetPx + p, y0, 1, minPx);
        continue;
      }
      ctx.fillRect(offsetPx + p, y0, 1, y1 - y0);
    }
  }
}

/** The upper and lower edge of each column's sample span, `thickness` device pixels; one line where the span is thinner. */
function paintSpanOutline(ctx, cols, { from, count, offsetPx, yCentre, signalPx, thickness, outer }) {
  for (let p = 0; p < count; p++) {
    const i = from + p;
    if (!(outer[p] > 0)) continue;
    const top = Math.round(yCentre - signalPx(cols.top[i]));
    const bottom = Math.round(yCentre - signalPx(cols.bottom[i]));
    if (bottom - top <= 2 * thickness) {
      const y = Math.round((top + bottom) / 2 - thickness / 2);
      ctx.fillRect(offsetPx + p, y, 1, Math.max(thickness, bottom - top));
      continue;
    }
    ctx.fillRect(offsetPx + p, top, 1, thickness);
    ctx.fillRect(offsetPx + p, bottom - thickness, 1, thickness);
  }
}

const mixers = new WeakMap();

/** One colour per column mixed from the band colours by their share of the column's energy (quantised so it stays a small set of strings). */
function bandMixer(palette) {
  let mixer = mixers.get(palette);
  if (mixer) return mixer;
  const rgb = [palette.low, palette.mid, palette.high].map(parseColour);
  const cache = new Map();
  const steps = 16;
  mixer = (low, mid, high) => {
    const sum = low * low + mid * mid + high * high;
    if (!(sum > 0)) return palette.low;
    const a = Math.round(((low * low) / sum) * steps);
    const b = Math.round(((mid * mid) / sum) * steps);
    const key = a * 64 + b;
    let colour = cache.get(key);
    if (!colour) {
      const wl = a / steps;
      const wm = b / steps;
      const wh = Math.max(0, 1 - wl - wm);
      const c = [0, 1, 2].map((k) => Math.round(rgb[0][k] * wl + rgb[1][k] * wm + rgb[2][k] * wh));
      colour = `rgb(${c[0]}, ${c[1]}, ${c[2]})`;
      cache.set(key, colour);
    }
    return colour;
  };
  mixers.set(palette, mixer);
  return mixer;
}

function parseColour(value) {
  const hex = /^#([0-9a-f]{6})$/i.exec(String(value).trim());
  if (hex) return [0, 2, 4].map((k) => parseInt(hex[1].slice(k, k + 2), 16));
  const rgb = /rgba?\(([^)]+)\)/.exec(String(value));
  if (rgb) return rgb[1].split(',').slice(0, 3).map((v) => Number(v.trim()));
  return [255, 255, 255];
}

/**
 * Dots on the samples once they are far enough apart to tell one from the next: their size follows the spacing, up to
 * DOT_MAX_RADIUS_CSS_PX. `sampleAt(k)` is sample k's value on the low band's 0..1 scale; `firstSample` / `msPerSample`
 * place sample k at `(firstSample + k) * msPerSample` ms, `startMs` is the view's left edge.
 */
export function paintSampleDots(ctx, { look = ZOOM_LOOK, startMs, msPerPx, msPerSample, sampleCount, sampleAt, firstSample, heightPx, palette, dpr = 1, widthPx }) {
  if (look === 'morph' || look === 'body') return;
  const pxPerSample = msPerSample / msPerPx;
  if (pxPerSample < DOTS_FROM_CSS_PX_PER_SAMPLE * dpr) return;
  const radius = Math.min(DOT_MAX_RADIUS_CSS_PX * dpr, Math.max(dpr, pxPerSample * DOT_RADIUS_PER_SAMPLE_PX));
  const yCentre = heightPx / 2;
  ctx.fillStyle = look === 'filled' ? palette.high : look === 'layered' ? palette.high : palette.mid;
  ctx.beginPath();
  for (let k = 0; k < sampleCount; k++) {
    const x = ((firstSample + k) * msPerSample - startMs) / msPerPx;
    if (x < -radius || x > widthPx + radius) continue;
    const v = sampleAt(k);
    const y = yCentre - Math.max(-1, Math.min(1, v * BAND_GAIN.low)) * yCentre;
    ctx.moveTo(x + radius, y);
    ctx.arc(x, y, radius, 0, Math.PI * 2);
  }
  ctx.fill();
}

/**
 * A stand-in for the few canvas calls the looks make (fillStyle, globalAlpha, save/restore, one-pixel-wide fillRect)
 * that writes the pixels of a `count` x `heightPx` strip into one ImageData on the waveform ground, put on the canvas
 * in one call. Thousands of one-pixel fillRects a frame are what made a zoom step slow to rasterise.
 */
class PixelColumns {
  static for(ctx, offsetPx, count, heightPx, palette) {
    let pool = pixelPools.get(ctx);
    if (!pool) {
      pool = new PixelColumns();
      pixelPools.set(ctx, pool);
    }
    pool.reset(ctx, offsetPx, count, heightPx, palette);
    return pool;
  }

  reset(ctx, offsetPx, count, heightPx, palette) {
    if (!this.image || this.image.width < count || this.image.height !== heightPx) {
      this.image = ctx.createImageData(Math.max(count, this.image?.height === heightPx ? this.image.width : 0), heightPx);
      this.words = new Uint32Array(this.image.data.buffer);
    }
    this.offsetPx = offsetPx;
    this.width = count;
    this.height = heightPx;
    const ground = packColour(palette.bg);
    const stride = this.image.width;
    for (let row = 0; row < heightPx; row++) this.words.fill(ground, row * stride, row * stride + count);
    this.colour = 0;
    this.alpha = 1;
    this.stack = [];
  }

  set fillStyle(value) {
    this.colour = packColour(value);
  }

  set globalAlpha(value) {
    this.alpha = Math.max(0, Math.min(1, value));
  }

  get globalAlpha() {
    return this.alpha;
  }

  save() {
    this.stack.push(this.alpha);
  }

  restore() {
    if (this.stack.length) this.alpha = this.stack.pop();
  }

  fillRect(x, y, w, h) {
    const x0 = Math.max(0, Math.round(x) - this.offsetPx);
    const x1 = Math.min(this.width, Math.round(x + w) - this.offsetPx);
    const y0 = Math.max(0, Math.round(y));
    const y1 = Math.min(this.height, Math.round(y + h));
    if (x1 <= x0 || y1 <= y0) return;
    const words = this.words;
    const stride = this.image.width;
    const colour = this.colour;
    if (this.alpha >= 1 && x1 - x0 === 1) {
      const end = y1 * stride + x0;
      for (let i = y0 * stride + x0; i < end; i += stride) words[i] = colour;
      return;
    }
    if (this.alpha >= 1) {
      for (let row = y0; row < y1; row++) {
        const at = row * stride;
        for (let col = x0; col < x1; col++) words[at + col] = colour;
      }
      return;
    }
    if (this.alpha <= 0) return;
    const a = this.alpha;
    for (let row = y0; row < y1; row++) {
      const at = row * stride;
      for (let col = x0; col < x1; col++) words[at + col] = blend(words[at + col], colour, a);
    }
  }

  flushTo(ctx) {
    ctx.putImageData(this.image, this.offsetPx, 0, 0, 0, this.width, this.height);
  }
}

const pixelPools = new WeakMap();
const packed = new Map();

/** A CSS colour as one opaque RGBA word in the byte order of ImageData on a little-endian machine. */
function packColour(value) {
  let word = packed.get(value);
  if (word === undefined) {
    const [r, g, b] = parseColour(value);
    word = ((255 << 24) | (b << 16) | (g << 8) | r) >>> 0;
    packed.set(value, word);
  }
  return word;
}

function blend(under, over, a) {
  const mix = (shift) => Math.round(((over >>> shift) & 255) * a + ((under >>> shift) & 255) * (1 - a));
  return ((255 << 24) | (mix(16) << 16) | (mix(8) << 8) | mix(0)) >>> 0;
}
