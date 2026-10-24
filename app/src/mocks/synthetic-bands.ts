import type { BandColumns } from '../services/waveform-bands.ts';

const SECTIONS = [
  { until: 0.07, name: 'intro' },
  { until: 0.22, name: 'build' },
  { until: 0.32, name: 'breakdown' },
  { until: 0.58, name: 'drop' },
  { until: 0.66, name: 'breakdown' },
  { until: 0.9, name: 'drop' },
  { until: 1.01, name: 'outro' },
] as const;

export const WIRE_CALIBRATION = 0.9;
const WIRE_FULL_SCALE = 127;

function mulberry32(seed: number) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function sectionAt(fraction: number): { name: string; progress: number } {
  let from = 0;
  for (const s of SECTIONS) {
    if (fraction < s.until) return { name: s.name, progress: (fraction - from) / (s.until - from) };
    from = s.until;
  }
  return { name: 'outro', progress: 1 };
}

/**
 * A dance track as three band envelopes: kick and sidechained bass in low, claps, stabs, pads and
 * vocal phrases in mid, hats and risers in high, with intro, build, breakdown, two drops and an outro.
 * Each band is normalised to its own peak, like the per-band scaling of a real 3-band waveform.
 */
export function syntheticBands(columns: number, seed: number, durationSecs: number, bpm: number): BandColumns {
  const rand = mulberry32(seed * 7919 + 13);
  const low = new Float32Array(columns);
  const mid = new Float32Array(columns);
  const high = new Float32Array(columns);
  const beatSecs = 60 / bpm;
  const vocalSeed = rand() * 10;

  for (let i = 0; i < columns; i++) {
    const fraction = i / columns;
    const t = fraction * durationSecs;
    const beat = t / beatSecs;
    const phase = beat - Math.floor(beat);
    const beatIndex = Math.floor(beat);
    const bar = Math.floor(beat / 4);
    const { name, progress } = sectionAt(fraction);
    const drop = name === 'drop';
    const breakdown = name === 'breakdown';
    const kickOn = drop || name === 'build' || name === 'outro' || (name === 'intro' && progress > 0.5);
    const outroFade = name === 'outro' ? 1 - progress : 1;
    const jitter = 0.9 + 0.1 * rand();

    const kick = kickOn ? Math.exp(-phase * 7) * outroFade : 0;
    const pump = 1 - Math.exp(-phase * 5);
    const bassOn = drop || (name === 'build' && progress > 0.3);
    const bass = bassOn ? (0.22 + 0.1 * Math.sin(bar * 1.7 + vocalSeed)) * pump : 0;
    const clap = beatIndex % 2 === 1 && (drop || name === 'build') ? Math.exp(-phase * 13) : 0;
    const stabPhase = (beat * 2) % 1;
    const stab = drop && bar % 2 === 0 && beatIndex % 4 !== 3 ? Math.exp(-stabPhase * 3.5) * 0.62 : 0;
    const pad = breakdown ? 0.4 + 0.12 * Math.sin(t * 0.6 + vocalSeed) : drop ? 0.26 : name === 'intro' ? 0.08 : 0.14;
    const vocalOn = (drop || breakdown) && (Math.floor(beat / 8) + Math.floor(vocalSeed)) % 3 !== 2;
    const vocal = vocalOn ? 0.42 * (0.55 + 0.45 * Math.abs(Math.sin(t * 9 + vocalSeed * 3))) * (0.7 + 0.3 * Math.sin(t * 1.3)) : 0;
    const hatOn = !(name === 'breakdown' && progress < 0.5);
    const closedHat = hatOn ? Math.exp(-Math.abs(phase - 0.5) * 26) * 0.55 : 0;
    const sixteenth = hatOn && (phase * 4) % 1 < 0.18 ? 0.16 : 0;
    const openHat = drop && beatIndex % 8 === 7 ? Math.exp(-Math.abs(phase - 0.5) * 6) * 0.5 : 0;
    const riser = name === 'build' || (breakdown && progress > 0.5) ? 0.5 * Math.pow(progress, 2) : 0;

    low[i] = Math.min(1, Math.max(kick, bass) * jitter);
    mid[i] = Math.min(1, (pad + vocal + stab + clap * 0.55) * jitter);
    high[i] = Math.min(1, (closedHat + sixteenth + openHat + clap * 0.35 + riser) * jitter);
  }

  for (const band of [low, mid, high]) {
    let peak = 0;
    for (let i = 0; i < columns; i++) if (band[i] > peak) peak = band[i];
    if (peak > 0) for (let i = 0; i < columns; i++) band[i] = (band[i] / peak) * WIRE_CALIBRATION;
  }
  return { low, mid, high };
}

/** Low, mid, high bytes per column, as the backend sends them (full scale 127, then calibration). */
export function bandBytes(bands: BandColumns): Uint8Array {
  const columns = bands.low.length;
  const bytes = new Uint8Array(columns * 3);
  for (let i = 0; i < columns; i++) {
    bytes[i * 3] = Math.min(WIRE_FULL_SCALE, Math.round((bands.low[i] / WIRE_CALIBRATION) * WIRE_FULL_SCALE));
    bytes[i * 3 + 1] = Math.min(WIRE_FULL_SCALE, Math.round((bands.mid[i] / WIRE_CALIBRATION) * WIRE_FULL_SCALE));
    bytes[i * 3 + 2] = Math.min(WIRE_FULL_SCALE, Math.round((bands.high[i] / WIRE_CALIBRATION) * WIRE_FULL_SCALE));
  }
  return bytes;
}

function bytesToBase64(bytes: Uint8Array): string {
  const chunks: string[] = [];
  for (let i = 0; i < bytes.length; i += 0x2000) {
    chunks.push(String.fromCharCode.apply(null, bytes.subarray(i, i + 0x2000) as unknown as number[]));
  }
  return btoa(chunks.join(''));
}

export function encodeWire(bytes: Uint8Array) {
  return { len: bytes.length / 3, calibration: WIRE_CALIBRATION, bands: bytesToBase64(bytes) };
}

/** The wire shape of a track stored with 3-band export data: PWV7-style bytes, unscaled, flagged `pwv7`. */
export function encodeStickWire(bytes: Uint8Array) {
  return { len: bytes.length / 3, calibration: 1, pwv7: true, bands: bytesToBase64(bytes) };
}

export const ROW_BAND_COLUMNS = 128;

/** The backend's row preview: the bands averaged to 128 columns, as base64 low, mid, high bytes. */
export function rowBandsBase64(bands: BandColumns, columns = ROW_BAND_COLUMNS): string {
  const source = bands.low.length;
  const mean = (values: Float32Array, from: number, to: number) => {
    let sum = 0;
    for (let i = from; i < to; i++) sum += values[i];
    return sum / (to - from);
  };
  const reduced: BandColumns = { low: new Float32Array(columns), mid: new Float32Array(columns), high: new Float32Array(columns) };
  for (let c = 0; c < columns; c++) {
    const from = Math.floor((c * source) / columns);
    const to = Math.max(from + 1, Math.floor(((c + 1) * source) / columns));
    reduced.low[c] = mean(bands.low, from, to);
    reduced.mid[c] = mean(bands.mid, from, to);
    reduced.high[c] = mean(bands.high, from, to);
  }
  return bytesToBase64(bandBytes(reduced));
}

/** 400-byte PWAV preview: 5-bit height (peak) and 3-bit whiteness (how bright the window is). */
export function previewFromBands(bands: BandColumns, length = 400): number[] {
  const columns = bands.low.length;
  const out = new Array<number>(length);
  for (let p = 0; p < length; p++) {
    const from = Math.floor((p * columns) / length);
    const to = Math.max(from + 1, Math.floor(((p + 1) * columns) / length));
    let peak = 0;
    let l = 0;
    let m = 0;
    let h = 0;
    for (let i = from; i < to; i++) {
      peak = Math.max(peak, bands.low[i], bands.mid[i], bands.high[i]);
      l += bands.low[i];
      m += bands.mid[i];
      h += bands.high[i];
    }
    const total = l + m + h;
    const brightness = total > 0 ? (0.5 * m + h) / total : 0;
    const height = Math.max(1, Math.round((peak / WIRE_CALIBRATION) * 31));
    const white = Math.max(0, Math.min(7, Math.round(brightness * 1.4 * 7)));
    out[p] = (white << 5) | Math.min(31, height);
  }
  return out;
}
