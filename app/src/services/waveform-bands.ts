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

/** The same columns as bytes (`scale` brings them to 0..1): the 2400 columns per second series stays at one byte per column and band. */
export interface ByteBandColumns {
  low: Uint8Array;
  mid: Uint8Array;
  high: Uint8Array;
  scale: { low: number; mid: number; high: number };
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
const STRIP_LEVEL_PERCENTILE = 0.95;
const STRIP_LAYER_SHARE: Record<'low' | 'mid' | 'high', number> = { low: 0.4, mid: 0.2, high: 0.3 };
const STRIP_MIN_GAIN = 0.2;
const STRIP_MAX_GAIN = 4;
const STRIP_GAIN_STEPS = 14;

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

const DETAIL_MAGIC = 'FFWD';
const DETAIL_VERSION = 3;
const DETAIL_HEADER_BYTES = 28;

/** The 2400 columns per second series: bands plus the lowest and highest sample of each column. */
export interface DetailBandColumns extends ByteBandColumns {
  signalLow: Int8Array;
  signalHigh: Int8Array;
  signalScale: number;
  /** Bytes of the signal planes per full-scale sample (the low band's gain), for windows measured later. */
  signalGain: number;
}

/**
 * The high-resolution series of `get_waveform_detail` (`FFWD` header with the track's band gains and signal gain, then
 * the low, mid and high planes, bytes on the PWV7 scale, and the lowest and highest sample planes on the low band's
 * scale). The planes are views into `buffer`, drawn as `byte / 127`; nothing is copied or widened.
 */
export function bandsFromDetailBytes(buffer: ArrayBuffer): DetailBandColumns | null {
  if (buffer.byteLength < DETAIL_HEADER_BYTES) return null;
  const view = new DataView(buffer);
  const magic = String.fromCharCode(view.getUint8(0), view.getUint8(1), view.getUint8(2), view.getUint8(3));
  const columns = view.getUint32(12, true);
  if (magic !== DETAIL_MAGIC || view.getUint8(4) !== DETAIL_VERSION || buffer.byteLength !== DETAIL_HEADER_BYTES + 5 * columns) return null;
  if (columns === 0) return null;
  const at = (k: number) => DETAIL_HEADER_BYTES + k * columns;
  const plane = (k: number) => new Uint8Array(buffer, at(k), columns);
  const unit = 1 / WIRE_FULL_SCALE;
  return {
    low: plane(0),
    mid: plane(1),
    high: plane(2),
    scale: { low: unit, mid: unit, high: unit },
    signalLow: new Int8Array(buffer, at(3), columns),
    signalHigh: new Int8Array(buffer, at(4), columns),
    signalScale: unit,
    signalGain: view.getFloat32(24, true),
  };
}

/** A fine stretch of the zoom view (`get_waveform_window`): band bytes plus the lowest and highest sample per column. */
export interface WaveformWindow extends ByteBandColumns {
  startMs: number;
  endMs: number;
  sampleRate: number;
  /** Lowest and highest mono sample of each column, full scale 32767. */
  lowest: Int16Array;
  highest: Int16Array;
}

const WINDOW_MAGIC = 'FFWW';
const WINDOW_HEADER_BYTES = 32;

/** `FFWW` bytes of `get_waveform_window` as views into `buffer`, or null for a damaged or foreign buffer. */
export function windowFromBytes(buffer: ArrayBuffer): WaveformWindow | null {
  if (buffer.byteLength < WINDOW_HEADER_BYTES) return null;
  const view = new DataView(buffer);
  const magic = String.fromCharCode(view.getUint8(0), view.getUint8(1), view.getUint8(2), view.getUint8(3));
  const columns = view.getUint32(8, true);
  if (magic !== WINDOW_MAGIC || view.getUint8(4) !== 1 || columns === 0) return null;
  if (buffer.byteLength !== WINDOW_HEADER_BYTES + 7 * columns) return null;
  const startMs = view.getFloat64(16, true) * 1000;
  const endMs = view.getFloat64(24, true) * 1000;
  if (!(endMs > startMs)) return null;
  const samples = WINDOW_HEADER_BYTES;
  const bands = samples + 4 * columns;
  const plane = (k: number) => new Uint8Array(buffer, bands + k * columns, columns);
  const unit = 1 / WIRE_FULL_SCALE;
  return {
    startMs,
    endMs,
    sampleRate: view.getUint32(12, true),
    lowest: new Int16Array(buffer, samples, columns),
    highest: new Int16Array(buffer, samples + 2 * columns, columns),
    low: plane(0),
    mid: plane(1),
    high: plane(2),
    scale: { low: unit, mid: unit, high: unit },
  };
}

function percentile(values: ArrayLike<number>, p: number): number {
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
 * The preview is a mean per column of whichever series the analysis stored (PWV6, or the detail when there is none),
 * and a stored row preview does not say which. So it gets the same per-band gains as the strip (`stripGains`): either
 * source ends up at the same level, and the row shows its high band the way the strip does.
 */
export function rowBandsFromBandBytes(bandPreview: ArrayLike<number> | null | undefined): BandColumns | null {
  return cachedRowBands(bandPreview, (bytes) => {
    const columns = bandsFromStickBytes(bytes);
    return withGains(columns, stripGains(columns));
  });
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

type BandName = 'low' | 'mid' | 'high';
export type BandGains = Record<BandName, number>;

function layerAt(value: number, name: BandName, gain: number): number {
  return Math.fround(STACK_GAIN[name] * Math.pow(displayAmp(value * gain), STACK_GAMMA[name]));
}

function bandGain(values: ArrayLike<number>, name: BandName): number {
  const sorted = Float64Array.from(values).sort();
  const loudEnd = sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * STRIP_LEVEL_PERCENTILE))];
  const target = STRIP_LAYER_SHARE[name];
  if (layerAt(loudEnd, name, STRIP_MAX_GAIN) <= target) return STRIP_MAX_GAIN;
  let lo = STRIP_MIN_GAIN;
  let hi = STRIP_MAX_GAIN;
  for (let step = 0; step < STRIP_GAIN_STEPS; step++) {
    const mid = (lo + hi) / 2;
    if (layerAt(loudEnd, name, mid) > target) hi = mid;
    else lo = mid;
  }
  return lo;
}

/**
 * One gain per band for the whole track: each band's loud end (its 95th percentile) gets its own share of the strip
 * (`STRIP_LAYER_SHARE`), so the thin high band is not swamped by the low end and a hi-hat section stands out as a
 * clearly taller white layer. Per track, like Rekordbox's own gains: a breakdown still stays thin against its drop,
 * and the shares add up to less than the strip, so the stack does not clip.
 */
export function stripGains(bands: BandColumns): BandGains {
  if (bands.low.length === 0) return { low: 1, mid: 1, high: 1 };
  return { low: bandGain(bands.low, 'low'), mid: bandGain(bands.mid, 'mid'), high: bandGain(bands.high, 'high') };
}

function withGains(bands: BandColumns, gains: BandGains): BandColumns {
  const out = emptyBands(bands.low.length);
  for (const name of ['low', 'mid', 'high'] as const) {
    for (let i = 0; i < out[name].length; i++) out[name][i] = bands[name][i] * gains[name];
  }
  return { ...out, overview: true };
}

const stripBands = new WeakMap<BandColumns, BandColumns>();

/**
 * The columns the player's overview strip draws for an analysis payload: the PWV6 overview series when the
 * payload has one, else the detail columns brought to overview resolution by their mean. Either way each band gets its
 * own gain (`stripGains`), so every track fits the strip and its highs read. Never the row preview: that one
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
  const memo = stripBands.get(source);
  if (memo) return memo;
  const columns = overview ? source : { ...meanReduced(source, STRIP_COLUMNS), overview: true };
  const strip = withGains(columns, stripGains(columns));
  stripBands.set(source, strip);
  return strip;
}
