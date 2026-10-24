import { describe, expect, it } from 'vitest';
import { decodeColorWaveform } from './analysis-data.ts';
import {
  balanceBands,
  bandsFromColorSamples,
  bandsFromInterleavedBytes,
  bandsFromPreviewBytes,
  colorBandsFromPayload,
  rowBandsFor,
} from './waveform-bands.ts';
import { bandBytes, encodeWire, previewFromBands, syntheticBands } from '../mocks/synthetic-bands.ts';

function wire(bytes: number[], calibration = 0.9) {
  return { len: bytes.length / 3, calibration, bands: Buffer.from(bytes).toString('base64') };
}

describe('bandsFromInterleavedBytes', () => {
  it('matches the old {amp, r, g, b} decode for every column', () => {
    const bytes: number[] = [];
    for (let i = 0; i < 40; i++) bytes.push((i * 7) % 128, (i * 3) % 90, (i * 11) % 60);
    bytes.push(0, 0, 0, 127, 127, 127, 127, 3, 0);
    const encoded = wire(bytes, 1.1);
    const viaSamples = bandsFromColorSamples(decodeColorWaveform(encoded)!);
    const direct = bandsFromInterleavedBytes(bytes, 127, 1.1);
    for (const band of ['low', 'mid', 'high'] as const) {
      expect(direct[band]).toHaveLength(viaSamples[band].length);
      direct[band].forEach((v, i) => expect(v).toBeCloseTo(viaSamples[band][i], 5));
    }
  });
  it('keeps band proportions and clamps the loudest band to 1', () => {
    const out = bandsFromInterleavedBytes([127, 63, 0], 127, 2);
    expect(out.low[0]).toBeCloseTo(1, 6);
    expect(out.mid[0]).toBeCloseTo(63 / 127, 3);
    expect(out.high[0]).toBe(0);
  });
  it('ignores trailing bytes that do not make a column', () => {
    expect(bandsFromInterleavedBytes([1, 2, 3, 4, 5], 127).low).toHaveLength(1);
  });
});

describe('balanceBands', () => {
  it('lifts a quiet band toward the top and leaves a balanced one alone', () => {
    const n = 500;
    const loud = Float32Array.from({ length: n }, (_, i) => 0.5 + 0.45 * ((i % 10) / 10));
    const quiet = Float32Array.from({ length: n }, (_, i) => 0.2 * ((i % 10) / 10));
    const out = balanceBands({ low: loud, mid: quiet, high: loud });
    const peak = (a: Float32Array) => Math.max(...a);
    expect(peak(out.mid)).toBeGreaterThan(peak(quiet) * 2);
    expect(peak(out.low)).toBeLessThanOrEqual(1);
    expect(peak(out.low)).toBeCloseTo(peak(loud), 0);
  });
  it('bounds the gain so an almost empty band is not blown up', () => {
    const n = 200;
    const faint = Float32Array.from({ length: n }, (_, i) => (i % 5 === 0 ? 0.05 : 0.01));
    const out = balanceBands({ low: faint, mid: faint, high: faint });
    expect(Math.max(...out.low)).toBeLessThanOrEqual(0.05 * 2.5 + 1e-6);
  });
});

describe('colorBandsFromPayload', () => {
  it('reads the wire format and the legacy sample arrays to the same shape', () => {
    const bands = syntheticBands(600, 3, 200, 126);
    const encoded = encodeWire(bandBytes(bands));
    const fromWire = colorBandsFromPayload(encoded)!;
    const samples = decodeColorWaveform(encoded)!;
    const fromSamples = colorBandsFromPayload(samples)!;
    expect(fromWire.low).toHaveLength(600);
    fromWire.low.forEach((v, i) => expect(v).toBeCloseTo(fromSamples.low[i], 5));
  });
  it('is memoised per payload and rejects garbage', () => {
    const encoded = encodeWire(bandBytes(syntheticBands(100, 1, 100, 124)));
    expect(colorBandsFromPayload(encoded)).toBe(colorBandsFromPayload(encoded));
    expect(colorBandsFromPayload(null)).toBeNull();
    expect(colorBandsFromPayload({ len: 2 })).toBeNull();
    expect(colorBandsFromPayload('nope')).toBeNull();
  });
});

describe('bandsFromPreviewBytes', () => {
  const byte = (height: number, white: number) => (white << 5) | height;
  it('scales every band with the 5-bit height', () => {
    const out = bandsFromPreviewBytes([byte(0, 3), byte(31, 3)]);
    expect([out.low[0], out.mid[0], out.high[0]]).toEqual([0, 0, 0]);
    expect(out.low[1]).toBeGreaterThan(0);
    expect(bandsFromPreviewBytes([byte(16, 3)]).low[0]).toBeCloseTo(out.low[1] * (16 / 31), 5);
  });
  it('moves weight from low to high as whiteness grows', () => {
    const dark = bandsFromPreviewBytes([byte(31, 0)]);
    const white = bandsFromPreviewBytes([byte(31, 7)]);
    expect(dark.low[0]).toBeGreaterThan(dark.high[0]);
    expect(white.high[0]).toBeGreaterThan(white.low[0]);
    expect(white.high[0]).toBeGreaterThan(dark.high[0]);
    expect(white.low[0]).toBeLessThan(dark.low[0]);
  });
  it('keeps every value in 0..1 for every possible byte', () => {
    const all = Array.from({ length: 256 }, (_, i) => i);
    const out = bandsFromPreviewBytes(all);
    for (const band of [out.low, out.mid, out.high]) {
      for (const v of band) {
        expect(v).toBeGreaterThanOrEqual(0);
        expect(v).toBeLessThanOrEqual(1);
      }
    }
  });
});

describe('rowBandsFor', () => {
  it('returns null for missing or empty previews and the same object for the same preview', () => {
    expect(rowBandsFor(null)).toBeNull();
    expect(rowBandsFor(undefined)).toBeNull();
    expect(rowBandsFor(new Uint8Array(0))).toBeNull();
    const preview = new Uint8Array([31, 40, 90]);
    expect(rowBandsFor(preview)).toBe(rowBandsFor(preview));
    expect(rowBandsFor(preview)!.low).toHaveLength(3);
  });
  it('evicts the oldest entries beyond its limit', () => {
    const first = new Uint8Array([1, 2, 3]);
    const kept = rowBandsFor(first)!;
    for (let i = 0; i < 1100; i++) rowBandsFor(new Uint8Array([i % 31, 1, 2]));
    expect(rowBandsFor(first)).not.toBe(kept);
  });
});

describe('synthetic mock bands', () => {
  const bands = syntheticBands(3000, 5, 240, 124);
  it('stays in 0..1, is deterministic, and differs between seeds', () => {
    for (const band of [bands.low, bands.mid, bands.high]) {
      expect(Math.min(...band)).toBeGreaterThanOrEqual(0);
      expect(Math.max(...band)).toBeLessThanOrEqual(1);
    }
    expect(syntheticBands(3000, 5, 240, 124).low).toEqual(bands.low);
    expect(syntheticBands(3000, 6, 240, 124).mid).not.toEqual(bands.mid);
  });
  it('has a breakdown without kick and a drop with it', () => {
    const mean = (a: Float32Array, from: number, to: number) => a.slice(from, to).reduce((s, v) => s + v, 0) / (to - from);
    expect(mean(bands.low, 1000, 1500)).toBeGreaterThan(mean(bands.low, 740, 940) * 1.5);
    expect(mean(bands.mid, 740, 940)).toBeGreaterThan(mean(bands.mid, 60, 180));
  });
  it('encodes a preview whose whiteness varies with the spectrum', () => {
    const preview = previewFromBands(bands);
    expect(preview).toHaveLength(400);
    expect(new Set(preview.map((b) => b >> 5)).size).toBeGreaterThan(2);
  });
});
