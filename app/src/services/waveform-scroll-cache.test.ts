import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { drawBandRange } from '../../../prototyping/viewport/design-system/waveform-render.js';

const WIDTH = 200;
const HEIGHT = 200;
const palette = { low: 'LOW', mid: 'MID', high: 'HIGH', midInLow: 'M', highInLow: 'H', highInMid: 'HM', bg: 'BG', dim: 'DIM', ink: '0, 0, 0', playhead: 'P', version: 0 };

let columnsPainted = 0;

class FakeCanvas {
  width = 0;
  height = 0;
  private context = {
    canvas: this,
    fillStyle: '',
    fillRect() {
      if (this.fillStyle === 'LOW') columnsPainted++;
    },
    clearRect: () => {},
    drawImage: () => {},
  };
  getContext() {
    return this.context;
  }
}

beforeEach(() => {
  columnsPainted = 0;
  vi.stubGlobal('HTMLCanvasElement', FakeCanvas);
  vi.stubGlobal('document', { createElement: () => new FakeCanvas() });
});

afterEach(() => vi.unstubAllGlobals());

const bands = (columns: number) => ({
  low: new Float32Array(columns).fill(0.5),
  mid: new Float32Array(columns),
  high: new Float32Array(columns),
});

describe('scrolled view cache', () => {
  it('paints only the columns that came into view while the view scrolls', () => {
    const data = bands(1500);
    const main = new FakeCanvas();
    main.width = WIDTH;
    main.height = HEIGHT;
    const ctx = main.getContext();
    const visible = 0.1;
    const perFrame: number[] = [];
    for (let frame = 0; frame < 120; frame++) {
      const startFrac = 0.2 + frame * 0.0007;
      const before = columnsPainted;
      drawBandRange(ctx as unknown as CanvasRenderingContext2D, data, {
        startFrac,
        endFrac: startFrac + visible,
        widthPx: WIDTH,
        heightPx: HEIGHT,
        palette,
        scrolling: true,
      });
      perFrame.push(columnsPainted - before);
    }
    expect(perFrame[0]).toBeGreaterThanOrEqual(WIDTH);
    for (const painted of perFrame.slice(1)) expect(painted).toBeLessThanOrEqual(3);
  });

  it('repaints everything when the zoom really changes', () => {
    const data = bands(1500);
    const main = new FakeCanvas();
    main.width = WIDTH;
    main.height = HEIGHT;
    const ctx = main.getContext() as unknown as CanvasRenderingContext2D;
    const draw = (visible: number) =>
      drawBandRange(ctx, data, { startFrac: 0.2, endFrac: 0.2 + visible, widthPx: WIDTH, heightPx: HEIGHT, palette, scrolling: true });
    draw(0.1);
    columnsPainted = 0;
    draw(0.1);
    expect(columnsPainted).toBe(0);
    draw(0.12);
    expect(columnsPainted).toBeGreaterThanOrEqual(WIDTH);
  });
});
