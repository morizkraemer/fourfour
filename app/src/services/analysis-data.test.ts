import { describe, expect, it } from 'vitest';
import {
  decodeColorWaveform,
  decodeTrackPreview,
  isValidAnalysisPayload,
  isValidColorWaveform,
  isValidWaveformPreview,
  previewBytesFromPayload,
  previewPeaksNormalized,
  sanitizeAnalysisPayload,
} from './analysis-data.ts';

const preview = (fill = 0x7f) => Array.from({ length: 400 }, () => fill);
const band = (over: Record<string, number> = {}) => ({ amp: 0.5, r: 0.1, g: 0.2, b: 0.3, ...over });
const colors = (n = 8) => Array.from({ length: n }, () => band());

describe('isValidWaveformPreview', () => {
  it('accepts 400 in-range bytes', () => {
    expect(isValidWaveformPreview(preview())).toBe(true);
  });
  it('rejects non-arrays and wrong lengths', () => {
    expect(isValidWaveformPreview(null)).toBe(false);
    expect(isValidWaveformPreview('x')).toBe(false);
    expect(isValidWaveformPreview(preview().slice(1))).toBe(false);
    expect(isValidWaveformPreview([...preview(), 1])).toBe(false);
  });
  it('rejects out-of-range, NaN and non-number entries', () => {
    expect(isValidWaveformPreview(Object.assign(preview(), { 3: 256 }))).toBe(false);
    expect(isValidWaveformPreview(Object.assign(preview(), { 3: -1 }))).toBe(false);
    expect(isValidWaveformPreview(Object.assign(preview(), { 3: NaN }))).toBe(false);
    expect(isValidWaveformPreview(Object.assign(preview(), { 3: '5' }))).toBe(false);
  });
  it('rejects the flat placeholder whose 5-bit heights are all zero', () => {
    expect(isValidWaveformPreview(preview(0))).toBe(false);
    expect(isValidWaveformPreview(preview(0xe0))).toBe(false);
  });
  it('accepts a single non-zero height among zeros', () => {
    expect(isValidWaveformPreview(Object.assign(preview(0), { 399: 1 }))).toBe(true);
  });
  it('accepts boundary byte 255', () => {
    expect(isValidWaveformPreview(preview(255))).toBe(true);
  });
});

describe('isValidColorWaveform', () => {
  it('accepts 8 well-formed bands', () => {
    expect(isValidColorWaveform(colors(8))).toBe(true);
  });
  it('rejects fewer than 8 bands and empty arrays', () => {
    expect(isValidColorWaveform(colors(7))).toBe(false);
    expect(isValidColorWaveform([])).toBe(false);
    expect(isValidColorWaveform(undefined)).toBe(false);
  });
  it('allows values up to 1.25 and rejects above or negative', () => {
    expect(isValidColorWaveform([...colors(7), band({ amp: 1.25 })])).toBe(true);
    expect(isValidColorWaveform([...colors(7), band({ amp: 1.26 })])).toBe(false);
    expect(isValidColorWaveform([...colors(7), band({ r: -0.01 })])).toBe(false);
  });
  it('rejects null entries, missing fields and non-finite values', () => {
    expect(isValidColorWaveform([...colors(7), null])).toBe(false);
    expect(isValidColorWaveform([...colors(7), { amp: 1, r: 1, g: 1 }])).toBe(false);
    expect(isValidColorWaveform([...colors(7), band({ g: Infinity })])).toBe(false);
  });
});

describe('analysis payload helpers', () => {
  it('requires a valid mono preview', () => {
    expect(isValidAnalysisPayload({ waveform_preview: preview() })).toBe(true);
    expect(isValidAnalysisPayload({ waveform_preview: [] })).toBe(false);
    expect(isValidAnalysisPayload(null)).toBe(false);
    expect(isValidAnalysisPayload(42)).toBe(false);
  });
  it('sanitize returns null for an invalid payload', () => {
    expect(sanitizeAnalysisPayload({ waveform_preview: preview(0) })).toBeNull();
  });
  it('sanitize keeps a payload with valid colors untouched', () => {
    const data = { waveform_preview: preview(), waveform_color: colors() };
    expect(sanitizeAnalysisPayload(data)).toBe(data);
  });
  it('sanitize nulls corrupt colors but keeps other fields', () => {
    const data = { waveform_preview: preview(), waveform_color: colors(3), bpm: 128 };
    const out = sanitizeAnalysisPayload(data);
    expect(out.waveform_color).toBeNull();
    expect(out.bpm).toBe(128);
    expect(data.waveform_color).toHaveLength(3);
  });
  it('previewPeaksNormalized maps the low 5 bits to 0..1', () => {
    expect(previewPeaksNormalized([0, 31, 0xff, 0x60 | 16])).toEqual([0, 1, 1, 16 / 31]);
    expect(previewPeaksNormalized([])).toEqual([]);
  });
  it('previewBytesFromPayload passes arrays through and converts typed arrays', () => {
    const arr = [1, 2];
    expect(previewBytesFromPayload({ waveform_preview: arr })).toBe(arr);
    const typed = new Uint8Array([3, 4]) as unknown as number[];
    expect(previewBytesFromPayload({ waveform_preview: typed })).toEqual([3, 4]);
  });
});

const b64 = (bytes: number[]) => Buffer.from(bytes).toString('base64');
const encoded = (bytes: number[], calibration = 1) => ({
  len: bytes.length / 3,
  calibration,
  bands: b64(bytes),
});
const flatBands = (columns: number, triple = [10, 20, 30]) =>
  Array.from({ length: columns }, () => triple).flat();

function rustCalibration(entries: number[][], previewBytes: number[]) {
  if (!previewBytes.some((b) => (b & 0x1f) !== 0)) return 1;
  const pwavPeak = Math.max(...previewBytes.map((b) => (b & 0x1f) / 31));
  const bandPeak = Math.max(...entries.map(([l, m, h]) => Math.max(l, m, h) / 127));
  return bandPeak > 1e-6 ? pwavPeak / bandPeak : 1;
}

describe('decodeColorWaveform', () => {
  it('passes valid decoded arrays through unchanged', () => {
    const arr = colors();
    expect(decodeColorWaveform(arr)).toBe(arr);
  });
  it('returns null for null, short, malformed and non-finite calibration payloads', () => {
    expect(decodeColorWaveform(null)).toBeNull();
    expect(decodeColorWaveform(undefined)).toBeNull();
    expect(decodeColorWaveform(encoded(flatBands(7)))).toBeNull();
    expect(decodeColorWaveform({ len: 8, calibration: NaN, bands: b64(flatBands(8)) })).toBeNull();
    expect(decodeColorWaveform({ len: 8, calibration: 1, bands: 5 })).toBeNull();
  });
  it('decodes one column per three bytes and drops trailing bytes', () => {
    const out = decodeColorWaveform(encoded([...flatBands(8), 9, 9]))!;
    expect(out).toHaveLength(8);
  });
  it('normalises colour channels by the column maximum', () => {
    const [first] = decodeColorWaveform(encoded(flatBands(8, [127, 64, 0])))!;
    expect(first.r).toBe(1);
    expect(first.g).toBeCloseTo(64 / 127, 10);
    expect(first.b).toBe(0);
  });
  it('derives amp from the column maximum scaled by calibration', () => {
    const [first] = decodeColorWaveform(encoded(flatBands(8, [20, 60, 40]), 0.5))!;
    expect(first.amp).toBeCloseTo((60 / 127) * 0.5, 10);
  });
  it('clamps amp at 1 when calibration pushes it above', () => {
    const [first] = decodeColorWaveform(encoded(flatBands(8, [127, 0, 0]), 3))!;
    expect(first.amp).toBe(1);
  });
  it('turns silent columns into zero amp and zero colour', () => {
    const [first] = decodeColorWaveform(encoded(flatBands(8, [0, 0, 0]), 2))!;
    expect(first).toEqual({ amp: 0, r: 0, g: 0, b: 0 });
  });
  it('memoises per payload object', () => {
    const payload = encoded(flatBands(8));
    expect(decodeColorWaveform(payload)).toBe(decodeColorWaveform(payload));
  });
  it('reproduces the Rust calibration: loudest column amp equals the PWAV peak', () => {
    const entries = [
      [10, 20, 30],
      [127, 40, 5],
      [60, 60, 60],
      ...Array.from({ length: 5 }, () => [1, 1, 1]),
    ];
    const calibration = rustCalibration(entries, preview(0x10 | 20));
    expect(calibration).toBeCloseTo(20 / 31, 10);
    const out = decodeColorWaveform(encoded(entries.flat(), calibration))!;
    expect(Math.max(...out.map((s) => s.amp))).toBeCloseTo(20 / 31, 10);
  });
  it('keeps a sanitized encoded payload intact', () => {
    const data = { waveform_preview: preview(), waveform_color: encoded(flatBands(8)) };
    expect(sanitizeAnalysisPayload(data)).toBe(data);
  });
  it('nulls an encoded payload shorter than 8 columns on sanitize', () => {
    const data = { waveform_preview: preview(), waveform_color: encoded(flatBands(4)) };
    expect(sanitizeAnalysisPayload(data).waveform_color).toBeNull();
  });
});

describe('decodeTrackPreview', () => {
  it('decodes base64 to bytes', () => {
    expect(decodeTrackPreview(b64([0, 1, 127, 255]))).toEqual([0, 1, 127, 255]);
  });
  it('round-trips a full 400-byte preview into a valid waveform preview', () => {
    const bytes = Array.from({ length: 400 }, (_, i) => (i % 31) + 1);
    const decoded = decodeTrackPreview(b64(bytes))!;
    expect(decoded).toEqual(bytes);
    expect(isValidWaveformPreview(decoded)).toBe(true);
  });
  it('passes arrays through and maps nullish to null', () => {
    const arr = [1, 2];
    expect(decodeTrackPreview(arr)).toBe(arr);
    expect(decodeTrackPreview(null)).toBeNull();
    expect(decodeTrackPreview(undefined)).toBeNull();
  });
  it('decodes the empty string to an empty array', () => {
    expect(decodeTrackPreview('')).toEqual([]);
  });
});
