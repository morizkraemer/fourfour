import { describe, expect, it } from 'vitest';
import {
  LOUDNESS_CURVE,
  LOUDNESS_GAIN,
  MIN_HALF_FRACTION,
  displayLevel,
  drawLoudness,
  halfHeights,
  loudnessEnvelope,
} from '../../../prototyping/viewport/design-system/waveform-simple.js';

const flat = (value: number, columns = 64) => ({
  low: new Float32Array(columns).fill(value),
  mid: new Float32Array(columns).fill(value),
  high: new Float32Array(columns).fill(value),
});

describe('loudnessEnvelope', () => {
  it('is the library-gained RMS of the three bands for a steady signal', () => {
    const env = loudnessEnvelope(flat(0.2));
    expect(env[32]).toBeCloseTo(Math.sqrt((3 * 0.04) / 2) * LOUDNESS_GAIN, 5);
  });

  it('smooths a single spike into neighbours without raising its peak', () => {
    const bands = flat(0);
    bands.low[32] = 0.4;
    const env = loudnessEnvelope(bands);
    const raw = Math.min(1, Math.sqrt(0.16 / 2) * LOUDNESS_GAIN);
    expect(env[32]).toBeLessThan(raw);
    expect(env[31]).toBeGreaterThan(0);
    expect(env[33]).toBeGreaterThan(0);
    expect(env[31]).toBeCloseTo(env[33], 6);
    expect(env[20]).toBe(0);
  });

  it('is absolute: a quiet track stays lower than a loud one, no per-track normalisation', () => {
    const quiet = loudnessEnvelope(flat(0.1));
    const loud = loudnessEnvelope(flat(0.5));
    expect(loud[10]).toBeGreaterThan(quiet[10] * 3);
  });

  it('is memoised per bands object', () => {
    const bands = flat(0.3);
    expect(loudnessEnvelope(bands)).toBe(loudnessEnvelope(bands));
  });

  it('never exceeds 1', () => {
    expect(Math.max(...loudnessEnvelope(flat(1)))).toBeLessThanOrEqual(1);
  });
});

describe('displayLevel', () => {
  it('applies the gentle curve and lifts quiet levels', () => {
    expect(displayLevel(0.25)).toBeCloseTo(Math.pow(0.25, LOUDNESS_CURVE), 6);
    expect(displayLevel(0.25)).toBeGreaterThan(0.25);
    expect(displayLevel(1)).toBeCloseTo(1, 6);
  });

  it('keeps a minimum silhouette for silence and clamps out-of-range input', () => {
    expect(displayLevel(0)).toBe(MIN_HALF_FRACTION);
    expect(displayLevel(-1)).toBe(MIN_HALF_FRACTION);
    expect(displayLevel(5)).toBeCloseTo(1, 6);
  });

  it('is monotonic', () => {
    let last = 0;
    for (let v = 0; v <= 1; v += 0.05) {
      expect(displayLevel(v)).toBeGreaterThanOrEqual(last);
      last = displayLevel(v);
    }
  });
});

describe('halfHeights', () => {
  it('returns exactly one value per device pixel, whatever the source length', () => {
    const values = loudnessEnvelope(flat(0.3, 128));
    for (const widthPx of [60, 120, 240, 600]) expect(halfHeights(values, widthPx, 28)).toHaveLength(widthPx);
  });

  it('interpolates between source columns (no plateaus when zoomed in)', () => {
    const bands = flat(0, 8);
    bands.low.fill(0.6, 4);
    const halves = halfHeights(loudnessEnvelope(bands), 160, 28);
    const distinct = new Set(Array.from(halves, (v) => v.toFixed(3)));
    expect(distinct.size).toBeGreaterThan(8);
  });

  it('scales with the canvas height', () => {
    const values = loudnessEnvelope(flat(0.3));
    expect(halfHeights(values, 10, 56)[5]).toBeCloseTo(halfHeights(values, 10, 28)[5] * 2, 5);
  });
});

describe('drawLoudness', () => {
  function recorder() {
    const calls: string[] = [];
    const ctx = new Proxy(
      {},
      {
        get: (_, name: string) => (...args: unknown[]) => {
          calls.push(`${name}:${args.length}`);
        },
        set: () => true,
      },
    );
    return { ctx, calls };
  }

  it('paints one filled mirrored path and draws nothing for missing data', () => {
    const { ctx, calls } = recorder();
    drawLoudness(ctx as unknown as CanvasRenderingContext2D, flat(0.3), { widthPx: 100, heightPx: 28, palette: { ink: '255, 255, 255' } });
    expect(calls.filter((c) => c.startsWith('fill:'))).toHaveLength(1);
    expect(calls.filter((c) => c.startsWith('lineTo:'))).toHaveLength(2 * 100 + 2);

    const empty = recorder();
    drawLoudness(empty.ctx as unknown as CanvasRenderingContext2D, null, { widthPx: 100, heightPx: 28, palette: { ink: '255, 255, 255' } });
    expect(empty.calls).toHaveLength(0);
  });
});
