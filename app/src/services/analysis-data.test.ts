import { describe, expect, it } from 'vitest';
import {
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
  it('sanitize empties corrupt colors but keeps other fields', () => {
    const data = { waveform_preview: preview(), waveform_color: colors(3), bpm: 128 };
    const out = sanitizeAnalysisPayload(data);
    expect(out.waveform_color).toEqual([]);
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
