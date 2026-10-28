import { describe, expect, it } from 'vitest';
import { BAND_GAIN } from '../../../prototyping/viewport/design-system/waveform-bands.js';
import {
  SHAPE_FROM_MS_PER_PX,
  SHAPE_FULL_MS_PER_PX,
  ZOOM_LOOK,
  ZOOM_LOOKS,
  paintSampleDots,
  paintZoom,
  shapeAmount,
} from '../../../prototyping/viewport/design-system/waveform-zoom.js';

const palette = { bg: '#000000', low: '#0000ff', mid: '#ff8000', high: '#ffffff', midInLow: '#804000', highInLow: '#eeeeee', highInMid: '#dddddd' };

function pixelContext(width: number, height: number) {
  const data = new Uint8ClampedArray(width * height * 4);
  const ctx = {
    createImageData: (w: number, h: number) => ({ width: w, height: h, data: new Uint8ClampedArray(w * h * 4) }),
    putImageData: (image: { width: number; data: Uint8ClampedArray }, dx: number, dy: number, sx: number, sy: number, sw: number, sh: number) => {
      for (let y = 0; y < sh; y++) {
        for (let x = 0; x < sw; x++) {
          for (let c = 0; c < 4; c++) data[((dy + y) * width + dx + x) * 4 + c] = image.data[((sy + y) * image.width + sx + x) * 4 + c];
        }
      }
    },
  };
  const rgb = (x: number, y: number) => Array.from(data.slice((y * width + x) * 4, (y * width + x) * 4 + 3));
  const column = (x: number) => {
    const lit: number[] = [];
    for (let y = 0; y < height; y++) if (rgb(x, y).some((v) => v > 0)) lit.push(y);
    return lit.length ? { top: lit[0], bottom: lit[lit.length - 1] } : null;
  };
  return { ctx, rgb, column };
}

const columns = (top: number[], bottom: number[], low = 0.8) => ({
  low: Float32Array.from(top.map(() => low)),
  mid: Float32Array.from(top.map(() => low / 2)),
  high: Float32Array.from(top.map(() => 0)),
  top: Float32Array.from(top),
  bottom: Float32Array.from(bottom),
  length: top.length,
});

describe('shapeAmount', () => {
  it('is 0 zoomed out, 1 zoomed in, and rises without a jump in between', () => {
    expect(shapeAmount(SHAPE_FROM_MS_PER_PX * 2)).toBe(0);
    expect(shapeAmount(SHAPE_FULL_MS_PER_PX / 2)).toBe(1);
    let last = 0;
    for (let ms = SHAPE_FROM_MS_PER_PX; ms >= SHAPE_FULL_MS_PER_PX; ms *= 0.95) {
      const s = shapeAmount(ms);
      expect(s).toBeGreaterThanOrEqual(last);
      expect(s - last).toBeLessThan(0.12);
      last = s;
    }
  });
});

describe('paintZoom', () => {
  it('draws the app look from the list of looks', () => {
    expect(ZOOM_LOOKS).toContain(ZOOM_LOOK);
  });
  it('fills the signal to the centre line when zoomed in (filled), so a small sample is a body, not a thin trace', () => {
    const { ctx, column } = pixelContext(2, 200);
    paintZoom(ctx, columns([0.5, 0.2], [0.3, -0.4]), { look: 'filled', heightPx: 200, palette, msPerPx: 0.01 });
    const first = column(0)!;
    expect(first.top).toBeCloseTo(100 - 0.5 * BAND_GAIN.low * 100, -1);
    expect(first.bottom).toBeGreaterThanOrEqual(99);
    const second = column(1)!;
    expect(second.bottom).toBeCloseTo(100 + 0.4 * BAND_GAIN.low * 100, -1);
    expect(second.top).toBeLessThanOrEqual(100 - 0.2 * BAND_GAIN.low * 100 + 1);
  });
  it('draws the symmetric band envelope when zoomed out, whatever the look', () => {
    for (const look of ['body', 'filled', 'signal', 'layered']) {
      const { ctx, column } = pixelContext(1, 200);
      paintZoom(ctx, columns([0.1], [-0.1], 0.6), { look, heightPx: 200, palette, msPerPx: 50 });
      const span = column(0)!;
      expect(Math.abs(100 - span.top - (span.bottom + 1 - 100))).toBeLessThanOrEqual(1);
      expect(span.bottom - span.top).toBeGreaterThan(80);
    }
  });
  it('keeps a body symmetric at every zoom (body)', () => {
    const { ctx, column } = pixelContext(1, 200);
    paintZoom(ctx, columns([0.5], [0.3]), { look: 'body', heightPx: 200, palette, msPerPx: 0.01 });
    const span = column(0)!;
    expect(Math.abs(100 - span.top - (span.bottom + 1 - 100))).toBeLessThanOrEqual(1);
  });
  it('paints the waveform ground under the strip it owns', () => {
    const { ctx, rgb } = pixelContext(1, 10);
    paintZoom(ctx, columns([0], [0], 0), { look: 'filled', heightPx: 10, palette: { ...palette, bg: '#102030' }, msPerPx: 0.01 });
    expect(rgb(0, 0)).toEqual([16, 32, 48]);
  });
});

describe('paintSampleDots', () => {
  const recorder = () => {
    const arcs: Array<{ x: number; r: number }> = [];
    const ctx = { beginPath() {}, moveTo() {}, fill() {}, arc: (x: number, _y: number, r: number) => arcs.push({ x, r }), fillStyle: '' };
    return { ctx, arcs };
  };
  const dots = (msPerPx: number) => {
    const { ctx, arcs } = recorder();
    paintSampleDots(ctx, {
      look: 'filled', startMs: 0, msPerPx, msPerSample: 1000 / 44100, sampleCount: 40, sampleAt: () => 0.5, firstSample: 0,
      heightPx: 100, palette, dpr: 1, widthPx: 2000,
    });
    return arcs;
  };
  it('shows no dots while the samples are closer than a few pixels', () => {
    expect(dots(1000 / 44100)).toHaveLength(0);
  });
  it('puts one dot on each sample, at its exact time, sized by the spacing and capped', () => {
    const near = dots(1000 / 44100 / 6);
    expect(near).toHaveLength(40);
    expect(near[1].x - near[0].x).toBeCloseTo(6, 6);
    const far = dots(1000 / 44100 / 200);
    expect(far[0].r).toBeLessThanOrEqual(2.5);
    expect(far[0].r).toBeGreaterThan(near[0].r);
  });
});
