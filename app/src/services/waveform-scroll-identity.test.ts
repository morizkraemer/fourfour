import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { drawBandRange } from '../../../prototyping/viewport/design-system/waveform-render.js';
import { ZOOM_LOOK } from '../../../prototyping/viewport/design-system/waveform-zoom.js';
import { ZOOM_PEAK_WEIGHT } from '../../../prototyping/viewport/design-system/waveform-bands.js';

const WIDTH = 300;
const HEIGHT = 120;
const palette = {
  low: '#2050ff',
  mid: '#ffa020',
  high: '#ffffff',
  midInLow: '#8060a0',
  highInLow: '#a0c0ff',
  highInMid: '#ffe0c0',
  bg: '#0d0d0d',
  dim: '#000000',
  ink: '255, 255, 255',
  playhead: '#ff0000',
  version: 0,
};

function hexWord(value: string) {
  const m = /^#([0-9a-f]{6})$/i.exec(value);
  if (!m) return 0;
  const [r, g, b] = [0, 2, 4].map((k) => parseInt(m[1].slice(k, k + 2), 16));
  return ((255 << 24) | (b << 16) | (g << 8) | r) >>> 0;
}

type Image = { width: number; height: number; data: Uint8ClampedArray };

class PixelCanvas {
  private w = 0;
  private h = 0;
  pixels = new Uint32Array(0);
  private readonly ctx: Record<string, unknown>;

  constructor(width = 0, height = 0) {
    this.resize(width, height);
    const canvas = this;
    this.ctx = {
      canvas,
      fillStyle: '',
      globalAlpha: 1,
      save() {},
      restore() {},
      setTransform() {},
      scale() {},
      fillRect(x: number, y: number, w: number, h: number) {
        canvas.fill(x, y, w, h, hexWord(this.fillStyle as string));
      },
      clearRect(x: number, y: number, w: number, h: number) {
        canvas.fill(x, y, w, h, 0);
      },
      createImageData(w: number, h: number): Image {
        return { width: w, height: h, data: new Uint8ClampedArray(w * h * 4) };
      },
      putImageData(img: Image, dx: number, dy: number, sx: number, sy: number, sw: number, sh: number) {
        const words = new Uint32Array(img.data.buffer);
        for (let y = 0; y < sh; y++) for (let x = 0; x < sw; x++) canvas.set(dx + sx + x, dy + sy + y, words[(sy + y) * img.width + sx + x]);
      },
      drawImage(src: PixelCanvas, ...a: number[]) {
        const [sx, sy, sw, sh, dx, dy] = a.length === 2 ? [0, 0, src.width, src.height, a[0], a[1]] : a;
        for (let y = 0; y < sh; y++) for (let x = 0; x < sw; x++) canvas.set(dx + x, dy + y, src.pixels[(sy + y) * src.width + sx + x]);
      },
    };
  }

  get width() {
    return this.w;
  }

  set width(value: number) {
    this.resize(value, this.h);
  }

  get height() {
    return this.h;
  }

  set height(value: number) {
    this.resize(this.w, value);
  }

  resize(width: number, height: number) {
    this.w = width;
    this.h = height;
    this.pixels = new Uint32Array(width * height);
  }

  set(x: number, y: number, word: number) {
    if (x >= 0 && y >= 0 && x < this.width && y < this.height) this.pixels[y * this.width + x] = word;
  }

  fill(x: number, y: number, w: number, h: number, word: number) {
    const y1 = Math.min(this.height, Math.round(y + h));
    const x1 = Math.min(this.width, Math.round(x + w));
    for (let row = Math.max(0, Math.round(y)); row < y1; row++) {
      for (let col = Math.max(0, Math.round(x)); col < x1; col++) this.pixels[row * this.width + col] = word;
    }
  }

  getContext() {
    return this.ctx;
  }
}

class CachingCanvas extends PixelCanvas {}

let layers = 0;

beforeEach(() => {
  layers = 0;
  vi.stubGlobal('HTMLCanvasElement', CachingCanvas);
  vi.stubGlobal('document', {
    createElement: () => {
      layers++;
      return new CachingCanvas();
    },
  });
});

afterEach(() => vi.unstubAllGlobals());

function fineSeries(columns: number) {
  const wave = (i: number, f: number) => Math.abs(Math.sin(i * f) * Math.sin(i * 0.0013));
  const low = new Float32Array(columns);
  const mid = new Float32Array(columns);
  const high = new Float32Array(columns);
  const signalHigh = new Float32Array(columns);
  const signalLow = new Float32Array(columns);
  for (let i = 0; i < columns; i++) {
    low[i] = wave(i, 0.031);
    mid[i] = wave(i, 0.17) * 0.7;
    high[i] = wave(i, 0.9) * 0.4;
    signalHigh[i] = low[i] * 0.8;
    signalLow[i] = -low[i] * 0.75;
  }
  return { low, mid, high, signalHigh, signalLow, signalScale: 1 };
}

function frameOptions(startFrac: number, visible: number, durationMs: number) {
  return {
    startFrac,
    endFrac: startFrac + visible,
    widthPx: WIDTH,
    heightPx: HEIGHT,
    palette,
    ground: palette.bg,
    dpr: 1,
    decay: 0,
    peakWeights: ZOOM_PEAK_WEIGHT,
    zoomLook: ZOOM_LOOK,
    msPerPx: (visible * durationMs) / WIDTH,
    scrolling: true,
    layout: 'centred',
  };
}

describe('scrolled high-resolution view', () => {
  it.each([0.02, 0.05])('draws the same pixels from the column ring as a full repaint while it scrolls (%s of the track visible)', (visible) => {
    const bands = fineSeries(200_000);
    expect((visible * bands.low.length) / WIDTH).toBeGreaterThan(1);
    const cached = new CachingCanvas(WIDTH, HEIGHT);
    let startFrac = 0.3;
    for (let frame = 0; frame < 90; frame++) {
      startFrac += visible / WIDTH * (frame % 5 === 4 ? 2.4 : 0.7 + (frame % 3) * 0.3);
      const opts = frameOptions(startFrac, visible, 180_000);
      const reference = new PixelCanvas(WIDTH, HEIGHT);
      drawBandRange(reference.getContext() as unknown as CanvasRenderingContext2D, bands, opts);
      drawBandRange(cached.getContext() as unknown as CanvasRenderingContext2D, bands, opts);
      let differing = 0;
      for (let i = 0; i < reference.pixels.length; i++) if (reference.pixels[i] !== cached.pixels[i]) differing++;
      expect(differing, `frame ${frame}`).toBe(0);
    }
    expect(layers, 'drawn through the column ring').toBe(1);
  });
});
