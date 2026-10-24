import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
  BAND_GAIN,
  BAND_ORDER,
  DISPLAY_FLOOR,
  bandsFromPeaks,
  displayAmp,
  halfHeight,
  reduceBands,
  smoothEnvelope,
} from '../../../prototyping/viewport/design-system/waveform-bands.js';
import { drawBandRange, paintBands } from '../../../prototyping/viewport/design-system/waveform-render.js';

const bands = (low: number[], mid = low.map(() => 0), high = low.map(() => 0)) => ({
  low: Float32Array.from(low),
  mid: Float32Array.from(mid),
  high: Float32Array.from(high),
});

describe('displayAmp', () => {
  it('keeps silence at the floor and full scale at 1', () => {
    expect(displayAmp(0)).toBeCloseTo(DISPLAY_FLOOR, 6);
    expect(displayAmp(1)).toBeCloseTo(1, 6);
  });
  it('is monotonic and clamps out-of-range input', () => {
    let last = -1;
    for (let v = 0; v <= 1; v += 0.05) {
      expect(displayAmp(v)).toBeGreaterThan(last);
      last = displayAmp(v);
    }
    expect(displayAmp(-3)).toBeCloseTo(displayAmp(0), 6);
    expect(displayAmp(7)).toBeCloseTo(displayAmp(1), 6);
  });
});

describe('reduceBands', () => {
  it('takes the peak of the covered columns when zoomed out', () => {
    const out = reduceBands(bands([0.1, 0.9, 0.2, 0.3, 0.8, 0.1, 0.0, 0.4]), 0, 1, 4);
    [0.9, 0.3, 0.8, 0.4].forEach((v, i) => expect(out.low[i]).toBeCloseTo(displayAmp(v), 5));
  });
  it('reduces each band independently', () => {
    const out = reduceBands(bands([1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0]), 0, 1, 4);
    expect(out.low[0]).toBeCloseTo(1, 5);
    expect(out.mid[0]).toBeCloseTo(DISPLAY_FLOOR, 5);
    expect(out.mid[1]).toBeCloseTo(1, 5);
    expect(out.high[2]).toBeCloseTo(1, 5);
    expect(out.high[3]).toBeCloseTo(DISPLAY_FLOOR, 5);
  });
  it('interpolates when zoomed in, so there are no plateaus', () => {
    const out = reduceBands(bands([0, 1]), 0, 1, 16);
    const ramp = Array.from(out.low);
    for (let i = 1; i < ramp.length; i++) expect(ramp[i]).toBeGreaterThanOrEqual(ramp[i - 1]);
    expect(new Set(ramp.map((v) => v.toFixed(4))).size).toBeGreaterThan(8);
  });
  it('leaves pixels outside the track blank and in-track pixels at or above the floor', () => {
    const out = reduceBands(bands([0, 0, 0, 0]), -0.5, 1.5, 8);
    expect(Array.from(out.low.slice(0, 2))).toEqual([0, 0]);
    expect(Array.from(out.low.slice(6))).toEqual([0, 0]);
    for (let i = 2; i < 6; i++) expect(out.low[i]).toBeGreaterThanOrEqual(DISPLAY_FLOOR - 1e-6);
  });
  it('gives the same value to the same absolute pixel whatever the scroll offset (no shimmer)', () => {
    const n = 5000;
    const data = bands(Array.from({ length: n }, (_, i) => (Math.sin(i * 0.37) + 1) / 2));
    const fracPerPx = 1 / 800;
    const a = reduceBands(data, 100 * fracPerPx, 900 * fracPerPx, 800);
    const b = reduceBands(data, 103 * fracPerPx, 903 * fracPerPx, 800);
    for (let k = 0; k < 790; k++) expect(b.low[k]).toBeCloseTo(a.low[k + 3], 5);
  });
  it('returns an empty result for empty input or a degenerate range', () => {
    expect(reduceBands(bands([]), 0, 1, 4).low.every((v: number) => v === 0)).toBe(true);
    expect(reduceBands(bands([1, 1]), 0.5, 0.5, 4).low.every((v: number) => v === 0)).toBe(true);
  });
});

describe('smoothEnvelope', () => {
  it('decays a transient at the given factor and never drops below the input', () => {
    const out = smoothEnvelope(Float32Array.from([0, 1, 0, 0, 0, 0.5, 0]), 0.5);
    expect(Array.from(out)).toEqual([0, 1, 0.5, 0.25, 0.125, 0.5, 0.25]);
  });
});

describe('halfHeight', () => {
  it('scales by band gain and half height, keeps zero at zero and draws at least half a pixel', () => {
    expect(halfHeight(0, 'low', 50)).toBe(0);
    expect(halfHeight(1, 'high', 50)).toBeCloseTo(50 * BAND_GAIN.high, 6);
    expect(halfHeight(1, 'low', 50)).toBeCloseTo(50 * BAND_GAIN.low, 6);
    expect(halfHeight(0.001, 'mid', 3)).toBe(0.5);
  });
});

describe('bandsFromPeaks', () => {
  it('turns mono peaks into a low-heavy mix of the same height', () => {
    const out = bandsFromPeaks([0, 0.5, 1]);
    expect(out.low[2]).toBe(1);
    expect(out.mid[2]).toBeLessThan(out.low[2]);
    expect(out.high[2]).toBeLessThan(out.mid[2]);
    expect(out.low[0]).toBe(0);
  });
});

function recordingContext() {
  const fills: Array<{ color: string; points: Array<[number, number]> }> = [];
  let current: Array<[number, number]> = [];
  let fillStyle = '';
  const ctx = {
    set fillStyle(v: string) {
      fillStyle = v;
    },
    get fillStyle() {
      return fillStyle;
    },
    beginPath: () => {
      current = [];
    },
    moveTo: (x: number, y: number) => current.push([x, y]),
    lineTo: (x: number, y: number) => current.push([x, y]),
    closePath: () => {},
    fill: () => fills.push({ color: fillStyle, points: current }),
    save: () => {},
    restore: () => {},
    translate: () => {},
  };
  return { ctx, fills };
}

const palette = { low: 'LOW', mid: 'MID', high: 'HIGH', bg: 'BG', dim: 'DIM', ink: '0, 0, 0', playhead: 'P', version: 0 };

describe('paintBands', () => {
  it('fills high, then mid, then low (low in front), symmetric about the centre line', () => {
    const shaped = {
      low: Float32Array.from([1, 0.5]),
      mid: Float32Array.from([0.8, 0.4]),
      high: Float32Array.from([0.6, 0.2]),
      length: 2,
    };
    const { ctx, fills } = recordingContext();
    paintBands(ctx, shaped, { heightPx: 100, palette });
    expect(fills.map((f) => f.color)).toEqual(['HIGH', 'MID', 'LOW']);
    expect(BAND_ORDER).toEqual(['high', 'mid', 'low']);
    for (const band of ['high', 'mid', 'low'] as const) {
      const fill = fills[BAND_ORDER.indexOf(band)];
      const top = fill.points.slice(1, 3);
      const bottom = fill.points.slice(-2).reverse();
      top.forEach(([x, y], i) => {
        const expected = halfHeight(shaped[band][i], band, 50);
        expect(x).toBeCloseTo(i + 0.5, 6);
        expect(50 - y).toBeCloseTo(expected, 6);
        expect(bottom[i][1] - 50).toBeCloseTo(expected, 6);
      });
    }
  });
});

describe('drawBandRange', () => {
  it('draws nothing for an empty range and three fills otherwise', () => {
    const empty = recordingContext();
    drawBandRange(empty.ctx, bands([1, 1]), { startFrac: 0.5, endFrac: 0.5, widthPx: 4, heightPx: 10, palette });
    expect(empty.fills).toHaveLength(0);
    const full = recordingContext();
    drawBandRange(full.ctx, bands([1, 1]), { startFrac: 0, endFrac: 1, widthPx: 4, heightPx: 10, palette });
    expect(full.fills).toHaveLength(3);
  });
  it('draws the same shape for the visible pixels whether or not the grid is scroll-aligned', () => {
    const data = bands(Array.from({ length: 400 }, (_, i) => ((i * 7) % 13) / 13));
    const still = recordingContext();
    drawBandRange(still.ctx, data, { startFrac: 0, endFrac: 1, widthPx: 100, heightPx: 40, palette, decay: 0 });
    const scrolled = recordingContext();
    drawBandRange(scrolled.ctx, data, { startFrac: 0, endFrac: 1, widthPx: 100, heightPx: 40, palette, decay: 0, scrolling: true });
    const lowStill = still.fills[2].points.slice(1, 99).map(([, y]) => y);
    const lowScrolled = scrolled.fills[2].points.slice(1, 99).map(([, y]) => y);
    expect(lowScrolled).toEqual(lowStill);
  });
});

describe('waveform tokens', () => {
  const css = readFileSync(new URL('../../../prototyping/viewport/design-system/tokens.css', import.meta.url), 'utf8');
  const block = (selector: string) => {
    const start = css.indexOf(selector);
    return css.slice(start, css.indexOf('}', start));
  };
  const token = (scope: string, name: string) =>
    new RegExp(`--ff-wave-${name}:\\s*(#[0-9a-fA-F]{6})`).exec(scope)![1];
  const luminance = (hex: string) => {
    const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255).map((c) =>
      c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4,
    );
    return 0.2126 * r + 0.7152 * g + 0.0722 * b;
  };
  const contrast = (a: string, b: string) => {
    const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
    return (hi + 0.05) / (lo + 0.05);
  };

  for (const [theme, selector] of [
    ['dark', ':root {'],
    ['light', ":root[data-theme='light']"],
  ] as const) {
    it(`keeps each band at least 3:1 against the ${theme} waveform background`, () => {
      const scope = block(selector);
      const bg = token(scope, 'bg');
      for (const name of ['low', 'mid', 'high']) {
        expect(contrast(token(scope, name), bg)).toBeGreaterThanOrEqual(3);
      }
    });
  }
});
