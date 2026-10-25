/*
 * 3-band waveform math, no DOM. Shared by every waveform surface.
 *
 * Data contract (BandColumns): three equally long, time-ordered arrays of
 * per-column band amplitudes in 0..1 (Float32Array, number[] or any ArrayLike):
 *
 *   { low, mid, high }
 *
 * Columns are evenly spaced over the whole track. Each band is its own envelope;
 * they are not weights of one shared height. An optional `scale` ({low, mid, high}) multiplies
 * the values before display (data whose full scale is below 1, such as the 128-column row preview).
 */

export const DISPLAY_FLOOR = 0.02;
export const DISPLAY_GAMMA = 1;

const BUCKET_EPSILON = 1e-6;
const UNIT_SCALE = { low: 1, mid: 1, high: 1 };

/**
 * Per-band display gain against one global reference (the same for every track and view, never a
 * per-track normalisation, so a quiet breakdown stays thin). Low is the outer layer and its bytes top
 * out lowest, so it is lifted; mid and high sit inside it.
 */
export const BAND_GAIN = { low: 0.8, mid: 0.82, high: 0.89 };

/**
 * Share of the loudest column in a pixel bucket that counts towards its height (the rest is the bucket
 * mean). Pure peak pooling returns the kick level for every pixel of a drop, which is the flat block;
 * the mean keeps drops, breakdowns and fills apart. High keeps more peak so hats stay spikes.
 */
export const BUCKET_PEAK_WEIGHT = { low: 0.2, mid: 0.3, high: 0.4 };

/**
 * Overview and compact strip, which Rekordbox draws as a bottom-anchored stack (low, then mid on top of it, then high on
 * top of that) from its 1200-column mean overview. The detail columns are reduced by their mean here, and `OVERVIEW_SCALE`
 * brings that mean to the level Rekordbox's own overview bytes have (about 0.6 / 0.75 / 2 of the detail mean).
 * `STACK_GAIN` is the share of the strip height one unit of that level adds to the stack.
 */
export const OVERVIEW_PEAK_WEIGHT = { low: 0, mid: 0, high: 0 };
export const OVERVIEW_SCALE = { low: 0.6, mid: 0.75, high: 2 };
export const STACK_GAIN = { low: 1.08, mid: 0.7, high: 1.35 };
export const STACK_GAMMA = { low: 1, mid: 1, high: 1.7 };

/** Back to front, as Rekordbox stacks them: low is the outer layer, mid in front of it, high the thin core on top. */
export const BAND_ORDER = ['low', 'mid', 'high'];

/** Raw 0..1 amplitude to display height (sqrt lift plus a minimum silhouette). */
export function displayAmp(raw) {
  const a = Math.max(0, Math.min(1, raw));
  return DISPLAY_FLOOR + (1 - DISPLAY_FLOOR) * Math.pow(a, DISPLAY_GAMMA);
}

function emptyShaped(pixels) {
  return { low: new Float32Array(pixels), mid: new Float32Array(pixels), high: new Float32Array(pixels), length: pixels };
}

/**
 * Reduce columns to one display height per pixel column over [startFrac, endFrac] of the track.
 * More than one source column per pixel: a blend of the peak and the mean of the covered columns (BUCKET_PEAK_WEIGHT). Fewer: linear interpolation at the pixel centre, so zoomed-in
 * views have no plateaus. Pixels outside the track (fraction < 0 or > 1) stay 0, in-track pixels are at
 * least DISPLAY_FLOOR.
 *
 * @returns {{low: Float32Array, mid: Float32Array, high: Float32Array, length: number}}
 */
export function reduceBands(bands, startFrac, endFrac, pixels, peakWeights = BUCKET_PEAK_WEIGHT, bandScale = null) {
  const out = emptyShaped(pixels);
  const n = bands && bands.low ? bands.low.length : 0;
  if (!n || pixels <= 0 || !(endFrac > startFrac)) return out;
  const perPixel = ((endFrac - startFrac) * n) / pixels;
  const first = startFrac * n;
  const scale = bands.scale ?? UNIT_SCALE;
  for (const name of BAND_ORDER) {
    const src = bands[name];
    const dst = out[name];
    for (let p = 0; p < pixels; p++) {
      const lo = first + p * perPixel;
      const hi = lo + perPixel;
      if (hi <= BUCKET_EPSILON || lo >= n - BUCKET_EPSILON) continue;
      let v;
      if (perPixel <= 1) {
        v = src[Math.max(0, Math.min(n - 1, Math.floor((lo + hi) / 2)))];
      } else {
        const i0 = Math.max(0, Math.floor(lo + BUCKET_EPSILON));
        const i1 = Math.min(n - 1, Math.ceil(hi - BUCKET_EPSILON) - 1);
        let peak = 0;
        let sum = 0;
        for (let i = i0; i <= i1; i++) {
          const x = src[i];
          sum += x;
          if (x > peak) peak = x;
        }
        const weight = peakWeights[name];
        v = weight * peak + (1 - weight) * (sum / (i1 - i0 + 1));
      }
      dst[p] = displayAmp(v * scale[name] * (bandScale ? bandScale[name] : 1));
    }
  }
  return out;
}

/**
 * Peak-hold follower along the pixel axis: a transient decays over the same number of
 * source columns whatever the zoom. `decayPerPixel` is the factor per output pixel.
 */
export function smoothEnvelope(values, decayPerPixel) {
  let env = 0;
  for (let i = 0; i < values.length; i++) {
    env = values[i] > env ? values[i] : env * decayPerPixel;
    if (env < values[i]) env = values[i];
    values[i] = env;
  }
  return values;
}

export function smoothBands(shaped, decayPerPixel) {
  for (const name of BAND_ORDER) smoothEnvelope(shaped[name], decayPerPixel);
  return shaped;
}

/** Half height in pixels of one band at one pixel column (0 stays 0, anything drawn is at least half a pixel). */
export function halfHeight(value, band, halfPx) {
  if (!(value > 0)) return 0;
  return Math.max(0.5, Math.min(1, value * BAND_GAIN[band]) * halfPx);
}

/** Heights of the three stacked layers (cumulative, in pixels from the baseline) at one pixel column. */
export function stackHeights(shaped, p, heightPx) {
  const low = shaped.low[p];
  if (!(low > 0)) return null;
  const layer = (name) => STACK_GAIN[name] * Math.pow(shaped[name][p], STACK_GAMMA[name]) * heightPx;
  const c1 = Math.min(heightPx, layer('low'));
  const c2 = Math.min(heightPx, c1 + layer('mid'));
  const c3 = Math.min(heightPx, c2 + layer('high'));
  return [Math.max(1, c1), Math.max(1, c2), Math.max(1, c3)];
}

/** Fallback for callers that only have a mono 0..1 peak array: a plausible band mix of the same height. */
export function bandsFromPeaks(peaks) {
  const n = peaks.length;
  const low = new Float32Array(n);
  const mid = new Float32Array(n);
  const high = new Float32Array(n);
  for (let i = 0; i < n; i++) {
    const p = Math.max(0, Math.min(1, peaks[i]));
    low[i] = p;
    mid[i] = p * 0.62;
    high[i] = p * 0.34;
  }
  return { low, mid, high };
}
