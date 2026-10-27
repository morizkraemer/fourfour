/**
 * The single place where analysis payloads become the renderer's input.
 *
 * The renderer (`$ds/waveform-render.js`) takes BandColumns: three equally long arrays of
 * per-column low/mid/high amplitudes in 0..1. Everything the app has today converts here:
 *
 *   - colour waveform from `get_analysis_data` (wire format `{len, calibration, bands}` or the
 *     older `{amp, r, g, b}[]` samples),
 *   - the 400-byte PWAV preview that every library row carries (height + whiteness only).
 *
 * Real 3-band columns (PWV7 or the filter bank) plug in through `bandsFromInterleavedBytes`.
 */
import { STACK_GAIN, STACK_GAMMA, displayAmp } from '../../../prototyping/viewport/design-system/waveform-bands.js';
import {
  decodeColorWaveform,
  type ColorWaveSample,
  type EncodedColorWaveform,
} from './analysis-data.ts';

export interface BandColumns {
  low: Float32Array;
  mid: Float32Array;
  high: Float32Array;
  scale?: { low: number; mid: number; high: number };
  /** The columns already are an overview series (PWV6 level), not detail data. */
  overview?: boolean;
}

const WIRE_FULL_SCALE = 127;
const BALANCE_BINS = 1024;
const BALANCE_PERCENTILE = 0.98;
const BALANCE_TARGET = 0.95;
const BALANCE_MIN_GAIN = 0.6;
const BALANCE_MAX_GAIN = 2.5;
const BALANCE_MIN_PEAK = 0.02;
const ROW_CACHE_LIMIT = 1000;
const LEGACY_LEVEL = 0.75;
const STRIP_COLUMNS = 1200;
const STRIP_HEADROOM_PERCENTILE = 0.95;
const STRIP_HEADROOM_LEVEL = 0.9;
const STRIP_GAIN_STEPS = 12;

const PREVIEW_MIX = Array.from({ length: 8 }, (_, level) => {
  const white = level / 7;
  return [1 - 0.85 * white, 0.35 + 0.5 * (1 - Math.abs(2 * white - 1)), 0.1 + 0.9 * Math.pow(white, 1.5)];
});

function emptyBands(columns: number): BandColumns {
  return { low: new Float32Array(columns), mid: new Float32Array(columns), high: new Float32Array(columns) };
}

/** Samples as drawn before the 3-band renderer: `amp` is the loudest band, r/g/b are weights relative to it. */
export function bandsFromColorSamples(samples: ColorWaveSample[], level = 1): BandColumns {
  const out = emptyBands(samples.length);
  for (let i = 0; i < samples.length; i++) {
    const s = samples[i];
    out.low[i] = s.amp * s.r * level;
    out.mid[i] = s.amp * s.g * level;
    out.high[i] = s.amp * s.b * level;
  }
  return out;
}

/**
 * Interleaved low, mid, high bytes per column. `fullScale` is the byte value that means amplitude 1;
 * `calibration` scales the loudest band of each column (the PWAV track-peak calibration), with the
 * result clamped to 1 and the band proportions kept.
 */
export function bandsFromInterleavedBytes(
  bytes: ArrayLike<number>,
  fullScale = WIRE_FULL_SCALE,
  calibration = 1,
): BandColumns {
  const columns = Math.floor(bytes.length / 3);
  const out = emptyBands(columns);
  for (let i = 0; i < columns; i++) {
    const low = bytes[i * 3];
    const mid = bytes[i * 3 + 1];
    const high = bytes[i * 3 + 2];
    const max = Math.max(low, mid, high);
    if (max === 0) continue;
    const gain = Math.min((max / fullScale) * calibration, 1) / max;
    out.low[i] = low * gain;
    out.mid[i] = mid * gain;
    out.high[i] = high * gain;
  }
  return out;
}

/** The columns a player shows for PWV7 / PWV6 bytes: `byte / 127` per band, nothing else applied. */
export function bandsFromStickBytes(bytes: ArrayLike<number>): BandColumns {
  return bandsFromInterleavedBytes(bytes, WIRE_FULL_SCALE, 1);
}

function percentile(values: Float32Array, p: number): number {
  const bins = new Uint32Array(BALANCE_BINS);
  for (let i = 0; i < values.length; i++) bins[Math.min(BALANCE_BINS - 1, Math.floor(values[i] * BALANCE_BINS))]++;
  const target = values.length * p;
  let seen = 0;
  for (let b = 0; b < BALANCE_BINS; b++) {
    seen += bins[b];
    if (seen >= target) return (b + 1) / BALANCE_BINS;
  }
  return 1;
}

/**
 * Not used for display any more (it flattened dynamics: every band's loud end sat at the top).
 * Scale each band so its 98th percentile lands near the top. Without it the loudest band of every
 * column (usually the low end) always reaches full height and hides the bands behind it.
 * Bounded gains keep a nearly empty band from being blown up.
 */
export function balanceBands(bands: BandColumns): BandColumns {
  const out = emptyBands(bands.low.length);
  for (const name of ['low', 'mid', 'high'] as const) {
    const src = bands[name];
    const peak = percentile(src, BALANCE_PERCENTILE);
    const gain =
      peak < BALANCE_MIN_PEAK ? 1 : Math.min(BALANCE_MAX_GAIN, Math.max(BALANCE_MIN_GAIN, BALANCE_TARGET / peak));
    const dst = out[name];
    for (let i = 0; i < src.length; i++) dst[i] = Math.min(1, src[i] * gain);
  }
  return out;
}

function isEncoded(color: unknown): color is EncodedColorWaveform {
  return !!color && typeof color === 'object' && !Array.isArray(color) && typeof (color as EncodedColorWaveform).bands === 'string';
}

function decodeBase64(text: string): Uint8Array {
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

const payloadBands = new WeakMap<object, BandColumns>();

/** Colour waveform from an analysis payload, in either wire shape. Memoised per payload object. */
export function colorBandsFromPayload(color: unknown): BandColumns | null {
  if (!color || typeof color !== 'object') return null;
  const memo = payloadBands.get(color);
  if (memo) return memo;
  let bands: BandColumns | null = null;
  if (isEncoded(color) && color.pwv7) {
    bands = bandsFromStickBytes(decodeBase64(color.bands));
    if (bands.low.length === 0) return null;
    payloadBands.set(color, bands);
    return bands;
  }
  if (isEncoded(color)) {
    bands = bandsFromInterleavedBytes(decodeBase64(color.bands), WIRE_FULL_SCALE, color.calibration * LEGACY_LEVEL);
  } else {
    const samples = decodeColorWaveform(color);
    if (samples) bands = bandsFromColorSamples(samples, LEGACY_LEVEL);
  }
  if (!bands || bands.low.length === 0) return null;
  payloadBands.set(color, bands);
  return bands;
}

/**
 * Approximate bands from a PWAV byte: 5-bit height and 3-bit whiteness. Whiteness is the only spectral
 * hint a preview carries (dark blue = bass-heavy, white = bright), so it steers the mix between the bands.
 */
export function bandsFromPreviewBytes(bytes: ArrayLike<number>): BandColumns {
  const out = emptyBands(bytes.length);
  for (let i = 0; i < bytes.length; i++) {
    const mix = PREVIEW_MIX[bytes[i] >> 5 & 0x07];
    const amp = (bytes[i] & 0x1f) / 31;
    out.low[i] = amp * mix[0];
    out.mid[i] = amp * mix[1];
    out.high[i] = amp * mix[2];
  }
  return out;
}

const rowCache = new Map<ArrayLike<number>, BandColumns>();

function cachedRowBands(
  bytes: ArrayLike<number> | null | undefined,
  convert: (bytes: ArrayLike<number>) => BandColumns,
): BandColumns | null {
  if (!bytes || bytes.length === 0) return null;
  const hit = rowCache.get(bytes);
  if (hit) {
    rowCache.delete(bytes);
    rowCache.set(bytes, hit);
    return hit;
  }
  const bands = convert(bytes);
  rowCache.set(bytes, bands);
  if (rowCache.size > ROW_CACHE_LIMIT) rowCache.delete(rowCache.keys().next().value!);
  return bands;
}

/** Row and detail previews from the 400-byte PWAV (tracks without band data). Small LRU: rows are drawn from a bitmap cache afterwards, so only visible tracks need bands. */
export function rowBandsFor(preview: ArrayLike<number> | null | undefined): BandColumns | null {
  return cachedRowBands(preview, bandsFromPreviewBytes);
}

/**
 * Row and detail previews from the 3-band row preview (`waveform_bands`: low, mid, high bytes per column).
 * The preview is the PWV6-sized mean per column, an overview series: the detail strip stacks it like the compact
 * strip does, at that level, and the row envelope reads the same columns.
 */
export function rowBandsFromBandBytes(bandPreview: ArrayLike<number> | null | undefined): BandColumns | null {
  return cachedRowBands(bandPreview, (bytes) => ({ ...bandsFromStickBytes(bytes), overview: true }));
}

const overviewBands = new WeakMap<object, BandColumns | null>();

/**
 * The overview series of an analysis payload (`waveform_overview`): Rekordbox's PWV6, or ours, as the stick carries it.
 * Only for payloads whose bytes are stick values (`pwv7`); older stored tracks have no such series and the
 * caller falls back to the detail columns.
 */
export function overviewBandsFromPayload(color: unknown): BandColumns | null {
  if (!color || typeof color !== 'object') return null;
  if (overviewBands.has(color)) return overviewBands.get(color) ?? null;
  const stick = isEncoded(color) && color.pwv7 ? colorBandsFromPayload(color) : null;
  const result = stick && stick.low.length >= 2 ? { ...stick, overview: true } : null;
  overviewBands.set(color, result);
  return result;
}

function meanReduced(bands: BandColumns, columns: number): BandColumns {
  const n = bands.low.length;
  if (n <= columns) return bands;
  const out = emptyBands(columns);
  for (const name of ['low', 'mid', 'high'] as const) {
    const src = bands[name];
    for (let c = 0; c < columns; c++) {
      const from = Math.floor((c * n) / columns);
      const to = Math.max(from + 1, Math.floor(((c + 1) * n) / columns));
      let sum = 0;
      for (let i = from; i < to; i++) sum += src[i];
      out[name][c] = sum / (to - from);
    }
  }
  return out;
}

function stackLevel(bands: BandColumns, i: number, gain: number): number {
  const layer = (name: 'low' | 'mid' | 'high') =>
    STACK_GAIN[name] * Math.pow(displayAmp(bands[name][i] * gain), STACK_GAMMA[name]);
  return layer('low') + layer('mid') + layer('high');
}

function stackPercentileLevel(bands: BandColumns, gain: number): number {
  const levels = new Float32Array(bands.low.length);
  for (let i = 0; i < levels.length; i++) levels[i] = stackLevel(bands, i, gain);
  levels.sort();
  return levels[Math.min(levels.length - 1, Math.floor(levels.length * STRIP_HEADROOM_PERCENTILE))];
}

/**
 * One gain for the whole track, never above 1: loud tracks are brought down until the 95th percentile of the stacked
 * height (low + mid + high) sits at STRIP_HEADROOM_LEVEL of the strip, so drops keep their silhouette and the high band
 * is not a white slab on the top edge. Quiet tracks are left alone (a breakdown stays thin). Band proportions are kept.
 */
export function stripGain(bands: BandColumns): number {
  if (bands.low.length === 0 || stackPercentileLevel(bands, 1) <= STRIP_HEADROOM_LEVEL) return 1;
  let lo = 0;
  let hi = 1;
  for (let step = 0; step < STRIP_GAIN_STEPS; step++) {
    const mid = (lo + hi) / 2;
    if (stackPercentileLevel(bands, mid) > STRIP_HEADROOM_LEVEL) hi = mid;
    else lo = mid;
  }
  return lo;
}

function withGain(bands: BandColumns, gain: number): BandColumns {
  if (gain === 1) return bands;
  const out = emptyBands(bands.low.length);
  for (const name of ['low', 'mid', 'high'] as const) {
    for (let i = 0; i < out[name].length; i++) out[name][i] = bands[name][i] * gain;
  }
  return { ...out, overview: true };
}

/**
 * The columns the player's overview strip draws for an analysis payload: the PWV6 overview series when the
 * payload has one, else the detail columns brought to overview resolution by their mean. Either way the track gets
 * one headroom gain (`stripGain`), so every track fits the strip without clipping. Never the row preview: that one
 * is cut from whichever series the analysis stored and carries no scale of its own.
 */
export function stripBandsFromAnalysis(
  analysis: { waveform_overview?: unknown; waveform_color_detail?: unknown; waveform_color?: unknown } | null | undefined,
): BandColumns | null {
  if (!analysis) return null;
  const overview = overviewBandsFromPayload(analysis.waveform_overview);
  const source =
    overview ??
    colorBandsFromPayload(analysis.waveform_color_detail) ??
    colorBandsFromPayload(analysis.waveform_color);
  if (!source) return null;
  const columns = overview ? source : { ...meanReduced(source, STRIP_COLUMNS), overview: true };
  return withGain(columns, stripGain(columns));
}
