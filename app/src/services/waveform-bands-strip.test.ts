import { describe, expect, it } from 'vitest';
import { rowBandsFromBandBytes, stripBandsFromAnalysis, stripGains } from './waveform-bands.ts';
import { mockAnalysisData } from '../mocks/analysis.ts';
import { MOCK_PROFILE_ORDER, bandBytes, encodeStickWire, overviewLevel, syntheticBands } from '../mocks/synthetic-bands.ts';
import { STACK_GAIN, STACK_GAMMA, displayAmp, reduceBands, stackHeights } from '../../../prototyping/viewport/design-system/waveform-bands.js';

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
    const gains = stripGains({ low: Float32Array.of(40 / 127, 40 / 127), mid: Float32Array.of(20 / 127, 20 / 127), high: Float32Array.of(0, 0) });
    expect(bands!.low[0]).toBeCloseTo((40 / 127) * gains.low);
    expect(bands!.mid[1]).toBeCloseTo((20 / 127) * gains.mid);
  });

  it('falls back to the detail columns, reduced to overview columns and marked as overview', () => {
    const bytes = Array.from({ length: 3000 * 3 }, () => 20);
    const bands = stripBandsFromAnalysis({ waveform_color_detail: wire(bytes) });
    expect(bands?.overview).toBe(true);
    expect(bands?.low.length).toBe(1200);
    expect(bands!.low[0]).toBeCloseTo(bands!.low[600]);
  });

  it('is null without any waveform', () => {
    expect(stripBandsFromAnalysis({})).toBeNull();
    expect(stripBandsFromAnalysis(null)).toBeNull();
  });
});

describe('strip band gains', () => {
  const layerP95 = (bands: { low: Float32Array; mid: Float32Array; high: Float32Array }, band: 'low' | 'mid' | 'high') => {
    const shaped = reduceBands(bands, 0, 1, bands.low.length, { low: 0, mid: 0, high: 0 }, null);
    const heights: number[] = [];
    for (let p = 0; p < bands.low.length; p++) {
      const stack = stackHeights(shaped, p, 1000);
      if (!stack) continue;
      heights.push(band === 'low' ? stack[0] : band === 'mid' ? stack[1] - stack[0] : stack[2] - stack[1]);
    }
    heights.sort((x, y) => x - y);
    return heights[Math.floor(heights.length * 0.95)] / 1000;
  };

  it('gives each band its own gain, so a thin high band is lifted and a loud low end brought down', () => {
    const loud = syntheticBands(1200, 3, 240, 124, 'loud');
    const gains = stripGains(loud);
    expect(gains.low).toBeLessThan(1);
    const bands = stripBandsFromAnalysis({ waveform_overview: encodeStickWire(overviewLevel(bandBytes(loud))) })!;
    expect(layerP95(bands, 'low')).toBeGreaterThan(0.3);
    expect(layerP95(bands, 'high')).toBeGreaterThan(0.2);
  });

  it('shows a hi-hat section as a clearly taller high layer than the same groove without hats', () => {
    const n = 1200;
    const bytes: number[] = [];
    for (let i = 0; i < n; i++) bytes.push(70, 30, i >= 400 && i < 800 ? 34 : 7);
    const bands = stripBandsFromAnalysis({ waveform_overview: wire(bytes) })!;
    const shaped = reduceBands(bands, 0, 1, n, { low: 0, mid: 0, high: 0 }, null);
    const highAt = (p: number) => {
      const stack = stackHeights(shaped, p, 100)!;
      return stack[2] - stack[1];
    };
    expect(highAt(600)).toBeGreaterThan(20);
    expect(highAt(600)).toBeGreaterThan(8 * Math.max(1, highAt(200)));
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

describe('row preview normalisation', () => {
  const saturated = Uint8Array.from({ length: 128 * 3 }, (_, i) => [127, 40, 20][i % 3]);

  it('brings a saturated detail-sourced row preview down like the strip does', () => {
    const row = rowBandsFromBandBytes(saturated)!;
    expect(row.overview).toBe(true);
    expect(Math.max(...row.low)).toBeLessThan(1);
    const strip = stripBandsFromAnalysis({ waveform_color_detail: wire(Array.from(saturated)) })!;
    expect(row.low[0]).toBeCloseTo(strip.low[0], 5);
    expect(row.mid[0]).toBeCloseTo(strip.mid[0], 5);
  });

  it('brings a quiet and a saturated row preview to the same level, like the strip', () => {
    const quiet = Uint8Array.from({ length: 128 * 3 }, (_, i) => [20, 10, 5][i % 3]);
    const row = rowBandsFromBandBytes(quiet)!;
    expect(row.low[0]).toBeCloseTo(rowBandsFromBandBytes(saturated)!.low[0], 2);
  });

  const p95 = (values: ArrayLike<number>) => Array.from(values).sort((x, y) => x - y)[Math.floor(values.length * 0.95)];

  it('row preview and strip of every profile stay within 10% of each other in the loud end of the low band', () => {
    for (const profile of MOCK_PROFILE_ORDER) {
      const analysis = mockAnalysisData(MOCK_PROFILE_ORDER.indexOf(profile) + 1, 200, 12800, 'Am');
      const strip = stripBandsFromAnalysis(analysis)!;
      const row = rowBandsFromBandBytes(Uint8Array.from(atob((analysis as any).waveform_row_bands), (c) => c.charCodeAt(0)))!;
      expect(Math.abs(p95(row.low) - p95(strip.low))).toBeLessThan(0.1);
    }
  });
});

describe('stripGains', () => {
  function sortedLayerGains(bands: { low: ArrayLike<number>; mid: ArrayLike<number>; high: ArrayLike<number> }) {
    const share = { low: 0.4, mid: 0.2, high: 0.3 } as const;
    const layerPercentile = (values: ArrayLike<number>, name: 'low' | 'mid' | 'high', gain: number) => {
      const layers = new Float32Array(values.length);
      for (let i = 0; i < layers.length; i++) layers[i] = STACK_GAIN[name] * Math.pow(displayAmp(values[i] * gain), STACK_GAMMA[name]);
      layers.sort();
      return layers[Math.min(layers.length - 1, Math.floor(layers.length * 0.95))];
    };
    const gain = (values: ArrayLike<number>, name: 'low' | 'mid' | 'high') => {
      if (layerPercentile(values, name, 4) <= share[name]) return 4;
      let lo = 0.2;
      let hi = 4;
      for (let step = 0; step < 14; step++) {
        const mid = (lo + hi) / 2;
        if (layerPercentile(values, name, mid) > share[name]) hi = mid;
        else lo = mid;
      }
      return lo;
    };
    return { low: gain(bands.low, 'low'), mid: gain(bands.mid, 'mid'), high: gain(bands.high, 'high') };
  }

  it('gives the gains of sorting every gained layer, from one sort per band', () => {
    for (const [seed, profile] of MOCK_PROFILE_ORDER.entries()) {
      const bands = syntheticBands(1200, seed + 3, 240, 126, profile);
      expect(stripGains(bands)).toEqual(sortedLayerGains(bands));
    }
    const bytes = { low: Uint8Array.from({ length: 999 }, (_, i) => (i * 37) % 128), mid: Uint8Array.from({ length: 999 }, (_, i) => (i * 11) % 90), high: new Uint8Array(999) };
    expect(stripGains(bytes)).toEqual(sortedLayerGains(bytes));
  });

  it('returns the same strip for the same analysis payload without measuring it again', () => {
    const analysis = mockAnalysisData(14, 240, 12600, '3A');
    expect(stripBandsFromAnalysis(analysis)).toBe(stripBandsFromAnalysis(analysis));
  });
});
