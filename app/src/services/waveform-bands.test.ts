import { describe, expect, it } from 'vitest';
import { decodeBandPreview, decodeColorWaveform } from './analysis-data.ts';
import {
  windowFromBytes,
  balanceBands,
  bandsFromColorSamples,
  bandsFromDetailBytes,
  bandsFromInterleavedBytes,
  bandsFromPreviewBytes,
  bandsFromStickBytes,
  colorBandsFromPayload,
  overviewBandsFromPayload,
  rowBandsFor,
  rowBandsFromBandBytes,
  stripGains,
} from './waveform-bands.ts';
import { displayAmp, reduceBands, ZOOM_PEAK_WEIGHT } from '../../../prototyping/viewport/design-system/waveform-bands.js';
import { ROW_BAND_COLUMNS, bandBytes, encodeWire, previewFromBands, rowBandsBase64, syntheticBands } from '../mocks/synthetic-bands.ts';

function wire(bytes: number[], calibration = 0.9) {
  return { len: bytes.length / 3, calibration, bands: btoa(String.fromCharCode(...bytes)) };
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

describe('PWV7 payloads (the stick bytes)', () => {
  const pwv7Bytes = Array.from({ length: 600 }, (_, i) => (i * 37 + (i % 3) * 11) % 128);
  const stick = (bytes: number[]) => ({ ...wire(bytes, 1), pwv7: true });

  it('maps to byte / 127 per band, the columns the stick draws', () => {
    const bands = colorBandsFromPayload(stick(pwv7Bytes))!;
    expect(bands.low).toHaveLength(200);
    for (let i = 0; i < 200; i++) {
      expect(bands.low[i]).toBeCloseTo(pwv7Bytes[i * 3] / 127, 6);
      expect(bands.mid[i]).toBeCloseTo(pwv7Bytes[i * 3 + 1] / 127, 6);
      expect(bands.high[i]).toBeCloseTo(pwv7Bytes[i * 3 + 2] / 127, 6);
    }
  });

  it('is not rescaled, unlike the calibrated legacy path', () => {
    const quiet = Array.from({ length: 300 }, (_, i) => (i % 3 === 0 ? 20 : 10));
    const stickBands = colorBandsFromPayload(stick(quiet))!;
    expect(Math.max(...stickBands.low)).toBeCloseTo(20 / 127, 6);
    const legacy = colorBandsFromPayload(wire(quiet, 0.9))!;
    expect(Math.max(...legacy.low)).toBeCloseTo((20 / 127) * 0.9 * 0.75, 5);
  });

  it('legacy payloads are not balanced: a quiet band stays quiet and loud against quiet columns keep their ratio', () => {
    const columns = 200;
    const bytes = Array.from({ length: columns * 3 }, (_, i) => {
      const column = Math.floor(i / 3);
      const loud = column % 2 === 0;
      return [loud ? 100 : 10, loud ? 60 : 6, 2][i % 3];
    });
    const legacy = colorBandsFromPayload(wire(bytes, 1))!;
    expect(legacy.low[0] / legacy.low[1]).toBeCloseTo(10, 5);
    expect(Math.max(...legacy.high)).toBeLessThan(0.05);
    expect(Math.max(...legacy.low)).toBeLessThan(0.8);
  });

  it('is the stick columns under the track band gains in the row preview', () => {
    const bytes = Uint8Array.from(pwv7Bytes);
    const { overview, ...columns } = rowBandsFromBandBytes(bytes)!;
    const stick = bandsFromStickBytes(bytes);
    const gains = stripGains(stick);
    for (const band of ['low', 'mid', 'high'] as const) {
      expect(gains[band]).toBeGreaterThan(0);
      stick[band].forEach((v, i) => expect(columns[band][i]).toBeCloseTo(v * gains[band], 5));
    }
    expect(overview).toBe(true);
    expect(rowBandsFromBandBytes(bytes)).toBe(rowBandsFromBandBytes(bytes));
  });

  it('survives the wire: base64 of 128 columns decodes to the bytes the backend sent', () => {
    const bands = syntheticBands(600, 2, 200, 124);
    const text = rowBandsBase64(bands);
    const decoded = decodeBandPreview(text)!;
    expect(decoded).toHaveLength(ROW_BAND_COLUMNS * 3);
    expect(decodeBandPreview('AAA=')).toBeNull();
    expect(decodeBandPreview('')).toBeNull();
    expect(decodeBandPreview(null)).toBeNull();
    expect(decodeBandPreview([1, 2, 3])).toEqual(Uint8Array.from([1, 2, 3]));
  });
});

describe('overviewBandsFromPayload', () => {
  it('marks stick values as an overview series and is memoised', () => {
    const payload = { ...wire([10, 20, 30, 40, 50, 60]), pwv7: true };
    const bands = overviewBandsFromPayload(payload)!;
    expect(bands.overview).toBe(true);
    expect(Array.from(bands.low)).toEqual([10 / 127, 40 / 127].map(Math.fround));
    expect(overviewBandsFromPayload(payload)).toBe(bands);
  });
  it('has no overview series for older stored tracks, other shapes or nothing', () => {
    expect(overviewBandsFromPayload(wire([10, 20, 30, 40, 50, 60]))).toBeNull();
    expect(overviewBandsFromPayload(null)).toBeNull();
    expect(overviewBandsFromPayload([{ amp: 1, r: 1, g: 1, b: 1 }])).toBeNull();
  });
});

function detailFile(
  low: number[],
  mid: number[],
  high: number[],
  signalLow = low.map((v) => -v),
  signalHigh = low,
): ArrayBuffer {
  const n = low.length;
  const buffer = new ArrayBuffer(28 + 5 * n);
  const view = new DataView(buffer);
  'FFWD'.split('').forEach((c, i) => view.setUint8(i, c.charCodeAt(0)));
  view.setUint8(4, 3);
  view.setUint32(8, 2400, true);
  view.setUint32(12, n, true);
  view.setUint16(16, 300, true);
  view.setFloat32(24, 379.5, true);
  new Uint8Array(buffer, 28).set([...low, ...mid, ...high]);
  new Int8Array(buffer, 28 + 3 * n).set([...signalLow, ...signalHigh]);
  return buffer;
}

function windowFile(startS: number, endS: number, lowest: number[], highest: number[], low: number[]): ArrayBuffer {
  const n = lowest.length;
  const buffer = new ArrayBuffer(32 + 7 * n);
  const view = new DataView(buffer);
  'FFWW'.split('').forEach((c, i) => view.setUint8(i, c.charCodeAt(0)));
  view.setUint8(4, 1);
  view.setUint32(8, n, true);
  view.setUint32(12, 44100, true);
  view.setFloat64(16, startS, true);
  view.setFloat64(24, endS, true);
  new Int16Array(buffer, 32, 2 * n).set([...lowest, ...highest]);
  new Uint8Array(buffer, 32 + 4 * n).set([...low, ...low.map(() => 0), ...low.map(() => 0)]);
  return buffer;
}

describe('windowFromBytes', () => {
  it('reads the range, the sample planes and the band planes', () => {
    const win = windowFromBytes(windowFile(1.5, 1.504, [-100, 0, 5], [200, 0, 5], [10, 20, 30]))!;
    expect(win.startMs).toBeCloseTo(1500, 9);
    expect(win.endMs).toBeCloseTo(1504, 9);
    expect(win.sampleRate).toBe(44100);
    expect(Array.from(win.lowest)).toEqual([-100, 0, 5]);
    expect(Array.from(win.highest)).toEqual([200, 0, 5]);
    expect(Array.from(win.low)).toEqual([10, 20, 30]);
    expect(Array.from(win.mid)).toEqual([0, 0, 0]);
    expect(win.scale.low).toBeCloseTo(1 / 127, 8);
  });
  it('rejects a damaged, empty or foreign buffer', () => {
    const good = windowFile(0, 1, [1], [2], [3]);
    expect(windowFromBytes(good.slice(0, good.byteLength - 1))).toBeNull();
    expect(windowFromBytes(windowFile(1, 1, [1], [2], [3]))).toBeNull();
    expect(windowFromBytes(detailFile([1], [1], [1]))).toBeNull();
  });
});

describe('bandsFromDetailBytes', () => {
  it('splits the planes into views of the buffer and scales bytes by 1 / 127', () => {
    const bands = bandsFromDetailBytes(detailFile([0, 127, 64], [1, 2, 3], [9, 8, 7]))!;
    expect(Array.from(bands.low)).toEqual([0, 127, 64]);
    expect(Array.from(bands.mid)).toEqual([1, 2, 3]);
    expect(Array.from(bands.high)).toEqual([9, 8, 7]);
    expect(bands.scale.low).toBeCloseTo(1 / 127, 8);
  });
  it('reads the signed lowest and highest sample planes and the signal gain', () => {
    const bands = bandsFromDetailBytes(detailFile([1, 2, 3], [0, 0, 0], [0, 0, 0], [-127, -5, 0], [3, 90, 127]))!;
    expect(Array.from(bands.signalLow)).toEqual([-127, -5, 0]);
    expect(Array.from(bands.signalHigh)).toEqual([3, 90, 127]);
    expect(bands.signalScale).toBeCloseTo(1 / 127, 8);
    expect(bands.signalGain).toBeCloseTo(379.5, 3);
  });
  it('reduces the signal to the extremes under each pixel, signed, on the band scale', () => {
    const bands = bandsFromDetailBytes(detailFile([1, 1, 1, 1], [0, 0, 0, 0], [0, 0, 0, 0], [-10, 20, -60, 5], [30, 40, -20, 50]))!;
    const two = reduceBands(bands, 0, 1, 2, ZOOM_PEAK_WEIGHT);
    expect(two.top![0]).toBeCloseTo(40 / 127, 6);
    expect(two.bottom![0]).toBeCloseTo(-10 / 127, 6);
    expect(two.top![1]).toBeCloseTo(50 / 127, 6);
    expect(two.bottom![1]).toBeCloseTo(-60 / 127, 6);
  });
  it('draws a pixel narrower than a column as the straight line between the column centres across it', () => {
    const bands = bandsFromDetailBytes(detailFile([1, 1, 1, 1], [0, 0, 0, 0], [0, 0, 0, 0], [-10, 20, -60, 5], [30, 40, -20, 50]))!;
    const eight = reduceBands(bands, 0, 1, 8, ZOOM_PEAK_WEIGHT);
    expect(eight.top![5]).toBeCloseTo(15 / 127, 6);
    expect(eight.bottom![5]).toBeCloseTo(-60 / 127, 6);
    expect(eight.top![4]).toBeCloseTo(10 / 127, 6);
    for (let p = 1; p < 7; p++) {
      expect(eight.bottom![p]).toBeLessThanOrEqual(eight.top![p - 1] + 1e-6);
      expect(eight.top![p]).toBeGreaterThanOrEqual(eight.bottom![p - 1] - 1e-6);
    }
  });
  it('rejects a damaged or foreign file', () => {
    const good = detailFile([1, 2], [1, 2], [1, 2]);
    expect(bandsFromDetailBytes(good.slice(0, good.byteLength - 1))).toBeNull();
    expect(bandsFromDetailBytes(new ArrayBuffer(4))).toBeNull();
    expect(bandsFromDetailBytes(detailFile([], [], []))).toBeNull();
  });
  it('maps one column to one pixel at one column per pixel, and peaks over many columns per pixel', () => {
    const bands = bandsFromDetailBytes(detailFile([10, 100, 20, 120], [0, 0, 0, 0], [0, 0, 0, 0]))!;
    const one = reduceBands(bands, 0, 1, 4, ZOOM_PEAK_WEIGHT);
    expect(Array.from(one.low)).toEqual([10, 100, 20, 120].map((v) => Math.fround(displayAmp(v / 127))));
    const two = reduceBands(bands, 0, 1, 2, ZOOM_PEAK_WEIGHT);
    const w = ZOOM_PEAK_WEIGHT.low;
    expect(two.low[1]).toBeCloseTo(displayAmp((w * 120 + (1 - w) * 70) / 127), 5);
  });
});
