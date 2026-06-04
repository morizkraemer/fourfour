/**
 * Canvas-2D waveform renderer for the player strip (and future expanded player).
 * Color + mono modes ported from pioneer-test-ui/frontend/app.js (815–965).
 */

export interface ColorSample {
  amp: number;
  r: number;
  g: number;
  b: number;
}

export interface WaveformCue {
  position: number;
  color?: string;
}

export type WaveformMode = 'color' | 'mono';

export interface WaveformRendererOptions {
  mode?: WaveformMode;
  background?: string;
  onScrub?: (progress: number) => void;
}

export class WaveformRenderer {
  #canvas: HTMLCanvasElement;
  #ctx: CanvasRenderingContext2D;
  #mode: WaveformMode = 'color';
  #background = '#141414';
  #colorData: ColorSample[] | null = null;
  #previewBytes: number[] | null = null;
  #progress = 0;
  #cues: WaveformCue[] = [];
  #onScrub?: (progress: number) => void;
  #dragging = false;

  constructor(canvas: HTMLCanvasElement, options: WaveformRendererOptions = {}) {
    this.#canvas = canvas;
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('WaveformRenderer: 2d context unavailable');
    this.#ctx = ctx;
    if (options.mode) this.#mode = options.mode;
    if (options.background) this.#background = options.background;
    this.#onScrub = options.onScrub;
    this.#bindPointer();
  }

  setColorData(data: ColorSample[] | null) {
    this.#colorData = data?.length ? data : null;
    if (this.#colorData) this.#mode = 'color';
    this.draw();
  }

  setPreviewBytes(bytes: number[] | null) {
    this.#previewBytes = bytes?.length ? bytes : null;
    if (!this.#colorData && this.#previewBytes) this.#mode = 'mono';
    this.draw();
  }

  setProgress(progress: number) {
    this.#progress = Math.max(0, Math.min(1, progress));
    this.draw();
  }

  setCues(cues: WaveformCue[]) {
    this.#cues = cues ?? [];
    this.draw();
  }

  setMode(mode: WaveformMode) {
    this.#mode = mode;
    this.draw();
  }

  resize() {
    this.draw();
  }

  destroy() {
    this.#canvas.removeEventListener('pointerdown', this.#onPointerDown);
    window.removeEventListener('pointermove', this.#onPointerMove);
    window.removeEventListener('pointerup', this.#onPointerUp);
  }

  draw() {
    const rect = this.#canvas.getBoundingClientRect();
    if (rect.width < 1 || rect.height < 1) return;

    const dpr = window.devicePixelRatio || 1;
    this.#canvas.width = Math.round(rect.width * dpr);
    this.#canvas.height = Math.round(rect.height * dpr);
    this.#ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    const w = rect.width;
    const h = rect.height;
    const ctx = this.#ctx;

    ctx.fillStyle = this.#background;
    ctx.fillRect(0, 0, w, h);

    const startFrac = 0;
    const endFrac = 1;

    if (this.#mode === 'color' && this.#colorData?.length) {
      this.#renderColor(ctx, w, h, this.#colorData, startFrac, endFrac);
    } else if (this.#previewBytes?.length) {
      this.#renderMono(ctx, w, h, this.#previewBytes, startFrac, endFrac);
    }

    this.#renderCues(ctx, w, h);
    this.#renderPlayhead(ctx, w, h);
  }

  #renderColor(
    ctx: CanvasRenderingContext2D,
    w: number,
    h: number,
    data: ColorSample[],
    startFrac: number,
    endFrac: number,
  ) {
    const startIdx = Math.floor(startFrac * data.length);
    const endIdx = Math.ceil(endFrac * data.length);
    const visibleCount = Math.max(1, endIdx - startIdx);
    const barWidth = w / visibleCount;
    const centerY = h / 2;

    for (let i = 0; i < visibleCount; i++) {
      const di = startIdx + i;
      if (di >= data.length) break;
      const { amp, r, g, b } = data[di];
      const barH = Math.min(amp * centerY, centerY);
      ctx.fillStyle = `rgb(${Math.round(r * 255)}, ${Math.round(g * 255)}, ${Math.round(b * 255)})`;
      ctx.fillRect(i * barWidth, centerY - barH, Math.max(barWidth, 1), barH * 2);
    }
  }

  #renderMono(
    ctx: CanvasRenderingContext2D,
    w: number,
    h: number,
    data: number[],
    startFrac: number,
    endFrac: number,
  ) {
    const startIdx = Math.floor(startFrac * data.length);
    const endIdx = Math.ceil(endFrac * data.length);
    const visibleCount = Math.max(1, endIdx - startIdx);
    const barWidth = w / visibleCount;
    const centerY = h / 2;

    for (let i = 0; i < visibleCount; i++) {
      const di = startIdx + i;
      if (di >= data.length) break;
      const byte = data[di];
      const height = (byte & 0x1f) / 31.0;
      const whiteness = ((byte >> 5) & 0x07) / 7.0;
      const barH = Math.min(height * centerY, centerY);
      const brightness = Math.round(100 + whiteness * 155);
      ctx.fillStyle = `rgb(${brightness}, ${brightness}, ${brightness})`;
      ctx.fillRect(i * barWidth, centerY - barH, Math.max(barWidth, 1), barH * 2);
    }
  }

  #renderCues(ctx: CanvasRenderingContext2D, w: number, h: number) {
    for (const cue of this.#cues) {
      const frac = Math.max(0, Math.min(1, cue.position ?? 0));
      const x = frac * w;
      const color = cue.color || '#4ade80';
      ctx.fillStyle = color;
      ctx.fillRect(x - 0.5, 0, 1, h);
      ctx.beginPath();
      ctx.arc(x, 2.5, 2.5, 0, Math.PI * 2);
      ctx.fill();
    }
  }

  #renderPlayhead(ctx: CanvasRenderingContext2D, w: number, h: number) {
    const x = this.#progress * w;
    ctx.fillStyle = getComputedStyle(document.documentElement)
      .getPropertyValue('--ff-accent')
      .trim() || '#4a9eff';
    ctx.fillRect(x - 0.5, 0, 1, h);
  }

  #progressFromEvent(clientX: number): number {
    const rect = this.#canvas.getBoundingClientRect();
    return Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
  }

  #onPointerDown = (e: PointerEvent) => {
    if (!this.#onScrub) return;
    this.#dragging = true;
    this.#canvas.setPointerCapture(e.pointerId);
    const p = this.#progressFromEvent(e.clientX);
    this.#onScrub(p);
    e.preventDefault();
  };

  #onPointerMove = (e: PointerEvent) => {
    if (!this.#dragging || !this.#onScrub) return;
    this.#onScrub(this.#progressFromEvent(e.clientX));
  };

  #onPointerUp = () => {
    this.#dragging = false;
  };

  #bindPointer() {
    this.#canvas.style.touchAction = 'none';
    this.#canvas.addEventListener('pointerdown', this.#onPointerDown);
    window.addEventListener('pointermove', this.#onPointerMove);
    window.addEventListener('pointerup', this.#onPointerUp);
  }
}
