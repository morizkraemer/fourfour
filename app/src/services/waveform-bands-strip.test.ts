import { describe, expect, it } from 'vitest';
import { stripBandsFromAnalysis, stripGain } from './waveform-bands.ts';
import { mockAnalysisData } from '../mocks/analysis.ts';
import { MOCK_PROFILE_ORDER, bandBytes, encodeStickWire, overviewLevel, syntheticBands } from '../mocks/synthetic-bands.ts';
import { reduceBands, stackHeights } from '../../../prototyping/viewport/design-system/waveform-bands.js';

function wire(bytes: number[]) {
  return { len: bytes.length / 3, calibration: 1, pwv7: true, bands: btoa(String.fromCharCode(...bytes)) };
}

function clippedShare(bands: NonNullable<ReturnType<typeof stripBandsFromAnalysis>>): number {
  const shaped = reduceBands(bands, 0, 1, 600, { low: 0, mid: 0, high: 0 }, null);
  let clipped = 0;
  let drawn = 0;
  for (let p = 0; p < 600; p++) {
    const stack = stackHeights(shaped, p, 100);
    if (!stack) continue;
    drawn++;
    if (stack[2] >= 99.5) clipped++;
  }
  return clipped / drawn;
}

describe('stripBandsFromAnalysis', () => {
  it('uses the overview series and marks it as overview', () => {
    const bands = stripBandsFromAnalysis({
      waveform_overview: wire([40, 20, 0, 40, 20, 0]),
      waveform_color_detail: wire([10, 10, 10, 10, 10, 10, 10, 10, 10]),
    });
    expect(bands?.overview).toBe(true);
    expect(bands?.low.length).toBe(2);
    expect(bands?.low[0]).toBeCloseTo(40 / 127);
  });

  it('falls back to the detail columns, reduced to overview columns and marked as overview', () => {
    const bytes = Array.from({ length: 3000 * 3 }, () => 20);
    const bands = stripBandsFromAnalysis({ waveform_color_detail: wire(bytes) });
    expect(bands?.overview).toBe(true);
    expect(bands?.low.length).toBe(1200);
    expect(bands?.low[0]).toBeCloseTo(20 / 127);
  });

  it('is null without any waveform', () => {
    expect(stripBandsFromAnalysis({})).toBeNull();
    expect(stripBandsFromAnalysis(null)).toBeNull();
  });
});

describe('strip headroom', () => {
  it('leaves quiet tracks at their own level', () => {
    const quiet = syntheticBands(1200, 3, 240, 124, 'ambient');
    expect(stripGain(quiet)).toBe(1);
  });

  it('brings a brickwalled track down and keeps the band proportions', () => {
    const loud = syntheticBands(1200, 3, 240, 124, 'loud');
    const gain = stripGain(loud);
    expect(gain).toBeLessThan(1);
    expect(gain).toBeGreaterThan(0.3);
    const bytes = overviewLevel(bandBytes(loud));
    const bands = stripBandsFromAnalysis({ waveform_overview: encodeStickWire(bytes) })!;
    expect(bands.low[100] / bands.high[100]).toBeCloseTo(bytes[300] / bytes[302]);
    expect(bands.low[100]).toBeCloseTo((bytes[300] / 127) * gain);
  });

  it('keeps every mock profile and both payload shapes under a tenth of columns clipped at the top', () => {
    for (const profile of MOCK_PROFILE_ORDER) {
      for (const seed of [1, 4, 9]) {
        const overview = encodeStickWire(overviewLevel(bandBytes(syntheticBands(1200, seed, 240, 124, profile))));
        const detail = encodeStickWire(bandBytes(syntheticBands(36000, seed, 240, 124, profile)));
        expect(clippedShare(stripBandsFromAnalysis({ waveform_overview: overview })!)).toBeLessThan(0.1);
        expect(clippedShare(stripBandsFromAnalysis({ waveform_color_detail: detail })!)).toBeLessThan(0.1);
      }
    }
  });

  it('handles the default mock analysis payload', () => {
    const bands = stripBandsFromAnalysis(mockAnalysisData(7, 200, 12800, 'Am'))!;
    expect(clippedShare(bands)).toBeLessThan(0.1);
  });
});
