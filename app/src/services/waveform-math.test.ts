import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
  ATTACK_RISE,
  BAND_GAIN,
  BAND_ORDER,
  BUCKET_PEAK_WEIGHT,
  DISPLAY_FLOOR,
  INTERPOLATE_BELOW,
  interpolateColumns,
  OVERVIEW_PEAK_WEIGHT,
  OVERVIEW_SCALE,
  STACK_GAIN,
  bandsFromPeaks,
  displayAmp,
  halfHeight,
  reduceBands,
  smoothEnvelope,
  stackHeights,
} from '../../../prototyping/viewport/design-system/waveform-bands.js';
import { drawBandRange, paintBands, paintStacked } from '../../../prototyping/viewport/design-system/waveform-render.js';

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
  it('blends the peak and the mean of the covered columns when zoomed out', () => {
    const out = reduceBands(bands([0.1, 0.9, 0.2, 0.3, 0.8, 0.1, 0.0, 0.4]), 0, 1, 4);
    const w = BUCKET_PEAK_WEIGHT.low;
    [[0.1, 0.9], [0.2, 0.3], [0.8, 0.1], [0.0, 0.4]].forEach(([a, b], i) => {
      const blended = w * Math.max(a, b) + (1 - w) * ((a + b) / 2);
      expect(out.low[i]).toBeCloseTo(displayAmp(blended), 5);
    });
  });
  it('keeps a drop and a breakdown apart when both contain the same kick peak (no flat block)', () => {
    const kickEveryFourth = (level: number) => Array.from({ length: 64 }, (_, i) => (i % 4 === 0 ? 0.8 : level));
    const drop = reduceBands(bands(kickEveryFourth(0.5)), 0, 1, 1);
    const breakdown = reduceBands(bands(kickEveryFourth(0.02)), 0, 1, 1);
    expect(drop.low[0] / breakdown.low[0]).toBeGreaterThan(1.5);
  });
  it('uses one global reference: the same level is the same height whatever else the track holds', () => {
    const quiet = reduceBands(bands([0.3, 0.3, 0.3, 0.3]), 0, 1, 4);
    const loud = reduceBands(bands([0.3, 0.3, 0.9, 0.9]), 0, 1, 4);
    expect(quiet.low[0]).toBeCloseTo(loud.low[0], 6);
  });
  it('applies the per-band scale of data whose full scale is below 1', () => {
    const base = bands([0.2, 0.2], [0.2, 0.2], [0.2, 0.2]);
    const scaled = { ...base, scale: { low: 2, mid: 3, high: 1 } };
    const out = reduceBands(scaled, 0, 1, 2);
    expect(out.low[0]).toBeCloseTo(displayAmp(0.4), 6);
    expect(out.mid[0]).toBeCloseTo(displayAmp(0.6), 6);
    expect(out.high[0]).toBeCloseTo(displayAmp(0.2), 6);
  });
  it('reduces each band independently', () => {
    const out = reduceBands(bands([1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0]), 0, 1, 4);
    expect(out.low[0]).toBeCloseTo(1, 5);
    expect(out.mid[0]).toBeCloseTo(DISPLAY_FLOOR, 5);
    expect(out.mid[1]).toBeCloseTo(1, 5);
    expect(out.high[2]).toBeCloseTo(1, 5);
    expect(out.high[3]).toBeCloseTo(DISPLAY_FLOOR, 5);
  });
  it('holds each source column over its pixels when zoomed in, so an attack stays a hard edge', () => {
    const out = reduceBands(bands([0, 1]), 0, 1, 16);
    const held = Array.from(out.low).map((v) => v.toFixed(4));
    expect(new Set(held).size).toBe(2);
    expect(held.slice(0, 8)).toEqual(Array(8).fill(held[0]));
    expect(held.slice(8)).toEqual(Array(8).fill(held[8]));
  });
  it('interpolates between column centres when a column spans more than two pixels, so a slope is a line, not stairs', () => {
    const out = reduceBands(bands([0.2, 0.3, 0.4, 0.5]), 0, 1, 32);
    const values = Array.from(out.low);
    for (let i = 1; i < values.length; i++) expect(values[i]).toBeGreaterThanOrEqual(values[i - 1] - 1e-6);
    expect(new Set(values.map((v) => v.toFixed(4))).size).toBeGreaterThan(24);
    const step = (values[20] - values[4]) / 16;
    for (let i = 5; i < 20; i++) expect(values[i] - values[i - 1]).toBeCloseTo(step, 4);
  });
  it('hits the column value at the column centre', () => {
    const data = bands([0.2, 0.3, 0.25, 0.3]);
    const out = reduceBands(data, 0, 1, 32);
    expect(out.low[12]).toBeCloseTo(displayAmp(0.25 * 1) * 1, 1);
    expect(interpolateColumns(Float32Array.from([0.2, 0.3]), 1.5, 2)).toBeCloseTo(0.3, 6);
    expect(interpolateColumns(Float32Array.from([0.2, 0.3]), 1, 2)).toBeCloseTo(0.25, 6);
  });
  it('keeps an attack as a hard step at the column edge and decays smoothly after it', () => {
    const data = bands([0.1, 0.1, 0.8, 0.6, 0.4]);
    const out = reduceBands(data, 0, 1, 40);
    const values = Array.from(out.low);
    const edge = 16;
    expect(values[edge - 1]).toBeCloseTo(displayAmp(0.1), 4);
    expect(values[edge]).toBeCloseTo(displayAmp(0.8), 4);
    for (let i = 20; i < 31; i++) expect(values[i]).toBeLessThan(values[i - 1]);
    expect(ATTACK_RISE).toBeLessThan(0.7);
  });
  it('takes a small rise as a slope, not a step', () => {
    const out = reduceBands(bands([0.2, 0.2 + ATTACK_RISE / 2]), 0, 1, 32);
    const values = Array.from(out.low);
    expect(new Set(values.map((v) => v.toFixed(4))).size).toBeGreaterThan(8);
  });
  it('keeps holding columns that are two pixels wide or narrower', () => {
    const out = reduceBands(bands([0.2, 0.4, 0.6, 0.8]), 0, 1, 8);
    expect(INTERPOLATE_BELOW).toBe(0.5);
    expect(Array.from(out.low).map((v) => v.toFixed(4))).toEqual(
      [0.2, 0.2, 0.4, 0.4, 0.6, 0.6, 0.8, 0.8].map((v) => displayAmp(v).toFixed(4)),
    );
  });
  it('reduces the overview with the mean of the covered columns, scaled to the overview level', () => {
    const out = reduceBands(bands([1, 0, 0, 0], [1, 0, 0, 0], [1, 0, 0, 0]), 0, 1, 1, OVERVIEW_PEAK_WEIGHT, OVERVIEW_SCALE);
    expect(out.low[0]).toBeCloseTo(displayAmp(0.25 * OVERVIEW_SCALE.low), 5);
    expect(out.mid[0]).toBeCloseTo(displayAmp(0.25 * OVERVIEW_SCALE.mid), 5);
    expect(out.high[0]).toBeCloseTo(displayAmp(0.25 * OVERVIEW_SCALE.high), 5);
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
    expect(halfHeight(1, 'mid', 50)).toBeCloseTo(50 * BAND_GAIN.mid, 6);
    expect(halfHeight(1, 'low', 50)).toBeCloseTo(50 * BAND_GAIN.low, 6);
    expect(halfHeight(0.5, 'low', 50)).toBeCloseTo(50 * BAND_GAIN.low * 0.5, 6);
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

type Rect = { color: string; x: number; y: number; w: number; h: number };

function recordingContext() {
  const rects: Rect[] = [];
  const paths: Array<{ color: string; points: Array<[number, number]> }> = [];
  let points: Array<[number, number]> = [];
  let fillStyle = '';
  const ctx = {
    set fillStyle(v: string) {
      fillStyle = v;
    },
    get fillStyle() {
      return fillStyle;
    },
    fillRect: (x: number, y: number, w: number, h: number) => rects.push({ color: fillStyle, x, y, w, h }),
    beginPath: () => {
      points = [];
    },
    moveTo: (x: number, y: number) => points.push([x, y]),
    lineTo: (x: number, y: number) => points.push([x, y]),
    closePath: () => {},
    fill: () => paths.push({ color: fillStyle, points }),
  };
  return { ctx, rects, paths };
}

const palette = { low: 'LOW', mid: 'MID', high: 'HIGH', midInLow: 'MIDINLOW', highInLow: 'HIGHINLOW', highInMid: 'HIGHINMID', bg: 'BG', dim: 'DIM', ink: '0, 0, 0', playhead: 'P', version: 0 };

describe('paintBands', () => {
  it('stacks like Rekordbox: low outer, mid inside it in the darker tone, high core inside that, symmetric about the centre line', () => {
    const shaped: Record<string, any> = {
      low: Float32Array.from([1, 0.52]),
      mid: Float32Array.from([0.8, 0.41]),
      high: Float32Array.from([0.6, 0.2]),
      length: 2,
    };
    const { ctx, rects } = recordingContext();
    paintBands(ctx, shaped, { heightPx: 200, palette });
    expect(rects.map((r) => r.color)).toEqual(['LOW', 'LOW', 'MIDINLOW', 'MIDINLOW', 'HIGHINMID', 'HIGHINMID']);
    expect(BAND_ORDER).toEqual(['low', 'mid', 'high']);
    const colour: Record<string, string> = { low: 'LOW', mid: 'MIDINLOW', high: 'HIGHINMID' };
    for (const band of BAND_ORDER) {
      [0, 1].forEach((column) => {
        const rect = rects.find((r) => r.color === colour[band] && r.x === column)!;
        const half = halfHeight(shaped[band][column], band, 100);
        expect(100 - rect.y).toBeCloseTo(half, 0);
        expect(rect.y + rect.h - 100).toBeCloseTo(half, 0);
        expect(rect.w).toBe(1);
      });
    }
  });
  it('keeps the bright tone only where a front band sticks out of the band behind it', () => {
    const shaped = { low: Float32Array.from([0.3]), mid: Float32Array.from([0.9]), high: Float32Array.from([0.1]), length: 1 };
    const { ctx, rects } = recordingContext();
    paintBands(ctx, shaped, { heightPx: 200, palette });
    expect(rects.map((r) => r.color)).toEqual(['MID', 'MIDINLOW', 'HIGHINMID']);
    const half = (name: string) => 100 - rects.find((r) => r.color === name)!.y;
    expect(half('MID')).toBeGreaterThan(half('MIDINLOW'));
    expect(half('MIDINLOW')).toBeCloseTo(100 * BAND_GAIN.low * 0.3, 0);
    expect(half('MIDINLOW')).toBeGreaterThan(half('HIGHINMID'));
  });
  it('does not paint a layer that a taller layer drawn after it covers', () => {
    const shaped = { low: Float32Array.from([0.3]), mid: Float32Array.from([0.5]), high: Float32Array.from([0.9]), length: 1 };
    const { ctx, rects } = recordingContext();
    paintBands(ctx, shaped, { heightPx: 200, palette });
    expect(rects.map((r) => r.color)).toEqual(['HIGH', 'HIGHINMID']);
  });
  it('skips silent columns and never draws less than one pixel', () => {
    const shaped = { low: Float32Array.from([0, 0.001]), mid: new Float32Array(2), high: new Float32Array(2), length: 2 };
    const { ctx, rects } = recordingContext();
    paintBands(ctx, shaped, { heightPx: 200, palette });
    expect(rects.length).toBeGreaterThan(0);
    expect(rects.every((r) => r.x === 1)).toBe(true);
    expect(rects[0].h).toBeGreaterThanOrEqual(1);
  });
});

describe('paintBands on short canvases', () => {
  it('fills one stepped path per visible layer, back to front, with whole-pixel edges', () => {
    const shaped = {
      low: Float32Array.from([1, 0.5]),
      mid: Float32Array.from([0.8, 0.4]),
      high: Float32Array.from([0.6, 0.2]),
      length: 2,
    };
    const { ctx, rects, paths } = recordingContext();
    paintBands(ctx, shaped, { heightPx: 28, palette });
    expect(rects).toHaveLength(0);
    expect(paths.map((p) => p.color)).toEqual(['LOW', 'MIDINLOW', 'HIGHINMID']);
    const low = paths[0].points;
    expect(low[2]).toEqual([0, 14 - Math.round(halfHeight(1, 'low', 14))]);
    expect(low[3][0]).toBe(1);
    for (const [x, y] of low) {
      expect(Number.isInteger(x)).toBe(true);
      expect(Number.isInteger(y)).toBe(true);
    }
  });
});

describe('paintStacked (overview, compact strip)', () => {
  it('draws high behind, then mid, then low in front, each from the baseline up', () => {
    const shaped = { low: Float32Array.from([0.5, 0.5]), mid: Float32Array.from([0.4, 0.4]), high: Float32Array.from([0.9, 0.2]), length: 2 };
    const { ctx, paths } = recordingContext();
    paintStacked(ctx, shaped, { heightPx: 40, palette });
    expect(paths.map((p) => p.color)).toEqual(['HIGH', 'MID', 'LOW']);
    for (const path of paths) for (const [, y] of path.points) expect(y).toBeLessThanOrEqual(40);
    const topAt = (color: string, column: number) => {
      const pts = paths.find((p) => p.color === color)!.points;
      return Math.min(...pts.filter(([x]) => x === column || x === column + 1).map(([, y]) => y));
    };
    expect(topAt('LOW', 0)).toBeGreaterThan(topAt('MID', 0));
    expect(topAt('MID', 0)).toBeGreaterThan(topAt('HIGH', 0));
    expect(stackHeights(shaped, 0, 40)![2]).toBeGreaterThan(stackHeights(shaped, 1, 40)![2]);
  });
  it('stacks cumulatively and never past the strip height', () => {
    const full = { low: Float32Array.from([1]), mid: Float32Array.from([1]), high: Float32Array.from([1]), length: 1 };
    const [c1, c2, c3] = stackHeights(full, 0, 40)!;
    expect(c1).toBeLessThanOrEqual(c2);
    expect(c2).toBeLessThanOrEqual(c3);
    expect(c3).toBe(40);
    expect(stackHeights({ ...full, low: Float32Array.from([0]) }, 0, 40)).toBeNull();
  });
  it('draws an overview series as it is and detail data scaled down to overview level', () => {
    const topOfLow = (data: object) => {
      const { ctx, paths } = recordingContext();
      drawBandRange(ctx, data, { startFrac: 0, endFrac: 1, widthPx: 4, heightPx: 100, palette: { ...palette, low: 'LOW' }, layout: 'stacked' });
      const low = paths.find((p) => p.color === 'LOW')!;
      return 100 - Math.min(...low.points.map(([, y]) => y));
    };
    const flat = (v: number) => ({ low: new Float32Array(4).fill(v), mid: new Float32Array(4), high: new Float32Array(4) });
    const asOverview = topOfLow({ ...flat(0.3), overview: true });
    const asDetail = topOfLow(flat(0.3));
    expect(asOverview).toBeCloseTo(Math.round(STACK_GAIN.low * displayAmp(0.3) * 100), 0);
    expect(asDetail).toBeCloseTo(Math.round(STACK_GAIN.low * displayAmp(0.3 * OVERVIEW_SCALE.low) * 100), 0);
    expect(asOverview).toBeGreaterThan(asDetail);
  });
  it('draws the overview from drawBandRange when asked for the stacked layout', () => {
    const { ctx, paths, rects } = recordingContext();
    drawBandRange(ctx, bands([0.5, 0.5, 0.5, 0.5], [0.3, 0.3, 0.3, 0.3], [0.1, 0.1, 0.1, 0.1]), {
      startFrac: 0, endFrac: 1, widthPx: 4, heightPx: 40, palette, layout: 'stacked',
    });
    expect(rects).toHaveLength(0);
    expect(paths.map((p) => p.color)).toEqual(['HIGH', 'MID', 'LOW']);
  });
});

describe('drawBandRange', () => {
  it('draws nothing for an empty range', () => {
    const empty = recordingContext();
    drawBandRange(empty.ctx, bands([1, 1]), { startFrac: 0.5, endFrac: 0.5, widthPx: 4, heightPx: 10, palette });
    expect(empty.rects).toHaveLength(0);
  });
  it('draws the same columns whether or not the grid is scroll-aligned', () => {
    const data = bands(Array.from({ length: 400 }, (_, i) => ((i * 7) % 13) / 13));
    const still = recordingContext();
    drawBandRange(still.ctx, data, { startFrac: 0, endFrac: 1, widthPx: 100, heightPx: 200, palette, decay: 0 });
    const scrolled = recordingContext();
    drawBandRange(scrolled.ctx, data, { startFrac: 0, endFrac: 1, widthPx: 100, heightPx: 200, palette, decay: 0, scrolling: true });
    const lows = (r: Rect[]) => r.filter((x) => x.color === 'LOW' && x.x >= 0 && x.x < 100).map((x) => [x.x, x.y, x.h]);
    expect(lows(scrolled.rects).slice(0, 98)).toEqual(lows(still.rects).slice(0, 98));
  });
  it('draws every scrolled column at a whole device pixel, whatever the sub-pixel phase', () => {
    const data = bands(Array.from({ length: 400 }, () => 0.5));
    for (const startFrac of [0.00375, 0.0041, 0.00444, 0.0049]) {
      const { ctx, rects } = recordingContext();
      drawBandRange(ctx, data, { startFrac, endFrac: startFrac + 0.25, widthPx: 100, heightPx: 200, palette, decay: 0, scrolling: true });
      expect(rects.length).toBeGreaterThan(0);
      for (const r of rects) {
        expect(Number.isInteger(r.x)).toBe(true);
        expect(Number.isInteger(r.y)).toBe(true);
        expect(Number.isInteger(r.h)).toBe(true);
      }
    }
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
