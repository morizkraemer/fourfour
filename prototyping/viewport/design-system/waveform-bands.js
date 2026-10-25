/*
 * 3-band waveform math, no DOM. Shared by every waveform surface.
 *
 * Data contract (BandColumns): three equally long, time-ordered arrays of
 * per-column band amplitudes in 0..1 (Float32Array, number[] or any ArrayLike):
 *
 *   { low, mid, high }
 *
 * Columns are evenly spaced over the whole track. Each band is its own envelope;
 * they are not weights of one shared height.
 */

export const DISPLAY_FLOOR = 0.045;
export const DISPLAY_GAMMA = 0.65;

const BUCKET_EPSILON = 1e-6;

/** Per-band display gain. Low sits in front, so it is trimmed a little to keep mid and high visible. */
export const BAND_GAIN = { low: 0.72, mid: 0.9, high: 1 };

/** Back to front. The last band is drawn on top. */
export const BAND_ORDER = ['high', 'mid', 'low'];

/** Raw 0..1 amplitude to display height (sqrt lift plus a minimum silhouette). */
export function displayAmp(raw) {
  const a = Math.max(0, Math.min(1, raw));
  return DISPLAY_FLOOR + (1 - DISPLAY_FLOOR) * Math.pow(a, DISPLAY_GAMMA);
}

function emptyShaped(pixels) {
  return { low: new Float32Array(pixels), mid: new Float32Array(pixels), high: new Float32Array(pixels), length: pixels };
}

function sampleInterpolated(arr, n, centre) {
  const c = Math.max(0, Math.min(n - 1, centre));
  const i0 = Math.floor(c);
  const i1 = Math.min(n - 1, i0 + 1);
  const t = c - i0;
  return arr[i0] + (arr[i1] - arr[i0]) * t;
}

/**
 * Reduce columns to one display height per pixel column over [startFrac, endFrac] of the track.
 * More than one source column per pixel: the peak of the covered columns (min/max downsampling
 * of a magnitude envelope is its max). Fewer: linear interpolation at the pixel centre, so zoomed-in
 * views have no plateaus. Pixels outside the track (fraction < 0 or > 1) stay 0, in-track pixels are at
 * least DISPLAY_FLOOR.
 *
 * @returns {{low: Float32Array, mid: Float32Array, high: Float32Array, length: number}}
 */
export function reduceBands(bands, startFrac, endFrac, pixels) {
  const out = emptyShaped(pixels);
  const n = bands && bands.low ? bands.low.length : 0;
  if (!n || pixels <= 0 || !(endFrac > startFrac)) return out;
  const perPixel = ((endFrac - startFrac) * n) / pixels;
  const first = startFrac * n;
  for (const name of BAND_ORDER) {
    const src = bands[name];
    const dst = out[name];
    for (let p = 0; p < pixels; p++) {
      const lo = first + p * perPixel;
      const hi = lo + perPixel;
      if (hi <= BUCKET_EPSILON || lo >= n - BUCKET_EPSILON) continue;
      let v;
      if (perPixel <= 1) {
        v = sampleInterpolated(src, n, (lo + hi) / 2 - 0.5);
      } else {
        const i0 = Math.max(0, Math.floor(lo + BUCKET_EPSILON));
        const i1 = Math.min(n - 1, Math.ceil(hi - BUCKET_EPSILON) - 1);
        v = 0;
        for (let i = i0; i <= i1; i++) if (src[i] > v) v = src[i];
      }
      dst[p] = displayAmp(v);
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
