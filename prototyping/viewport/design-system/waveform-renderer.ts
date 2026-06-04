/**
 * Canvas-2D waveform renderer for the player strip (and future expanded player).
 * Color/mono waveform, beat + time grids, zoom/pan, scrub — ported from
 * pioneer-test-ui/frontend/app.js (815–1045).
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

export interface BeatMarker {
  time_ms: number;
  bar_position: number;
}

export type WaveformMode = 'color' | 'mono';

export interface WaveformRendererOptions {
  mode?: WaveformMode;
  background?: string;
  onScrub?: (progress: number) => void;
  followPlayhead?: boolean;
}

const ZOOM_MIN = 1;
const ZOOM_MAX = 64;
const DRAG_THRESHOLD_PX = 5;

export class WaveformRenderer {
  #canvas: HTMLCanvasElement;
  #ctx: CanvasRenderingContext2D;
  #mode: WaveformMode = 'color';
  #background = '#141414';
  #colorData: ColorSample[] | null = null;
  #previewBytes: number[] | null = null;
  #progress = 0;
  #cues: WaveformCue[] = [];
  #beats: BeatMarker[] = [];
  #durationMs = 0;
  #onScrub?: (progress: number) => void;
  #followPlayhead = true;

  #zoom = 1;
  #offset = 0;

  #pointerDown = false;
  #gestureActive = false;
  #dragStartX = 0;
  #dragStartY = 0;
  #dragStartZoom = 1;
  #dragStartOffset = 0;

  constructor(canvas: HTMLCanvasElement, options: WaveformRendererOptions = {}) {
    this.#canvas = canvas;
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('WaveformRenderer: 2d context unavailable');
    this.#ctx = ctx;
    if (options.mode) this.#mode = options.mode;
    if (options.background) this.#background = options.background;
    this.#onScrub = options.onScrub;
    if (options.followPlayhead != null) this.#followPlayhead = options.followPlayhead;
    this.#bindEvents();
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
    if (this.#followPlayhead) this.#ensurePlayheadVisible();
    this.draw();
  }

  setCues(cues: WaveformCue[]) {
    this.#cues = cues ?? [];
    this.draw();
  }

  setBeats(beats: BeatMarker[] | null) {
    this.#beats = beats?.length ? beats : [];
    this.draw();
  }

  setDurationMs(ms: number) {
    this.#durationMs = Math.max(0, ms);
    this.draw();
  }

  setMode(mode: WaveformMode) {
    this.#mode = mode;
    this.draw();
  }

  setFollowPlayhead(follow: boolean) {
    this.#followPlayhead = follow;
  }

  resetView() {
    this.#zoom = 1;
    this.#offset = 0;
    this.draw();
  }

  zoomIn() {
    this.#setZoom(this.#zoom * 1.25);
  }

  zoomOut() {
    this.#setZoom(this.#zoom / 1.25);
  }

  panByScreenFraction(dx: number) {
    if (this.#zoom <= 1) return;
    const rect = this.#canvas.getBoundingClientRect();
    const panAmount = (dx / rect.width) / this.#zoom;
    this.#offset = this.#clampOffset(this.#offset - panAmount);
    this.draw();
  }

  /** Handle keyboard shortcuts; returns true if consumed. */
  handleKey(e: KeyboardEvent): boolean {
    switch (e.key) {
      case '+':
      case '=':
        e.preventDefault();
        this.zoomIn();
        return true;
      case '-':
      case '_':
        e.preventDefault();
        this.zoomOut();
        return true;
      case '0':
        e.preventDefault();
        this.resetView();
        return true;
      case 'ArrowLeft':
        e.preventDefault();
        this.#onScrub?.(this.#seekDelta(e.shiftKey ? -4 : -1));
        return true;
      case 'ArrowRight':
        e.preventDefault();
        this.#onScrub?.(this.#seekDelta(e.shiftKey ? 4 : 1));
        return true;
      case 'Home':
        e.preventDefault();
        this.#onScrub?.(0);
        return true;
      case 'End':
        e.preventDefault();
        this.#onScrub?.(1);
        return true;
      default:
        return false;
    }
  }

  resize() {
    this.draw();
  }

  destroy() {
    this.#canvas.removeEventListener('pointerdown', this.#onPointerDown);
    this.#canvas.removeEventListener('wheel', this.#onWheel);
    window.removeEventListener('pointermove', this.#onPointerMove);
    window.removeEventListener('pointerup', this.#onPointerUp);
    window.removeEventListener('pointercancel', this.#onPointerUp);
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
    const { startFrac, endFrac } = this.#viewRange();

    ctx.fillStyle = this.#background;
    ctx.fillRect(0, 0, w, h);

    if (this.#mode === 'color' && this.#colorData?.length) {
      this.#renderColor(ctx, w, h, this.#colorData, startFrac, endFrac);
    } else if (this.#previewBytes?.length) {
      this.#renderMono(ctx, w, h, this.#previewBytes, startFrac, endFrac);
    }

    this.#renderBeatGrid(ctx, w, h, startFrac, endFrac);
    this.#renderTimeGrid(ctx, w, h, startFrac, endFrac);
    this.#renderCues(ctx, w, h, startFrac, endFrac);
    this.#renderPlayhead(ctx, w, h, startFrac, endFrac);
  }

  #viewRange() {
    const visible = 1 / this.#zoom;
    const startFrac = this.#offset;
    return { startFrac, endFrac: startFrac + visible, visible };
  }

  #clampOffset(offset: number) {
    return Math.max(0, Math.min(1 - 1 / this.#zoom, offset));
  }

  #setZoom(next: number, anchorFrac?: number) {
    const prevZoom = this.#zoom;
    this.#zoom = Math.max(ZOOM_MIN, Math.min(ZOOM_MAX, next));
    if (anchorFrac != null && this.#zoom > 1) {
      const visible = 1 / this.#zoom;
      this.#offset = this.#clampOffset(anchorFrac - visible * 0.5);
    } else if (this.#zoom <= 1) {
      this.#offset = 0;
    } else {
      this.#offset = this.#clampOffset(this.#offset * (prevZoom / this.#zoom));
    }
    this.draw();
  }

  #progressAtClientX(clientX: number): number {
    const rect = this.#canvas.getBoundingClientRect();
    const { startFrac, visible } = this.#viewRange();
    const fracInView = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
    return Math.max(0, Math.min(1, startFrac + fracInView * visible));
  }

  #xForProgress(w: number, progress: number, startFrac: number, endFrac: number) {
    if (endFrac <= startFrac) return 0;
    return ((progress - startFrac) / (endFrac - startFrac)) * w;
  }

  #ensurePlayheadVisible() {
    if (this.#zoom <= 1) {
      this.#offset = 0;
      return;
    }
    const visible = 1 / this.#zoom;
    const p = this.#progress;
    if (p < this.#offset) {
      this.#offset = this.#clampOffset(p - visible * 0.15);
    } else if (p > this.#offset + visible) {
      this.#offset = this.#clampOffset(p - visible * 0.85);
    }
  }

  #seekDelta(beats: number): number {
    if (!this.#onScrub) return this.#progress;
    const dur = this.#durationMs;
    if (!dur) return this.#progress;

    if (this.#beats.length && beats !== 0) {
      const currentMs = this.#progress * dur;
      let bestIdx = 0;
      let bestDist = Infinity;
      for (let i = 0; i < this.#beats.length; i++) {
        const d = Math.abs(this.#beats[i].time_ms - currentMs);
        if (d < bestDist) {
          bestDist = d;
          bestIdx = i;
        }
      }
      const nextIdx = Math.max(0, Math.min(this.#beats.length - 1, bestIdx + beats));
      return this.#beats[nextIdx].time_ms / dur;
    }

    const stepMs = beats < 0 ? -1000 : 1000;
    return Math.max(0, Math.min(1, (this.#progress * dur + stepMs) / dur));
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

  #renderBeatGrid(
    ctx: CanvasRenderingContext2D,
    w: number,
    h: number,
    startFrac: number,
    endFrac: number,
  ) {
    if (!this.#beats.length || !this.#durationMs) return;

    let barNumber = 0;
    for (const beat of this.#beats) {
      const frac = beat.time_ms / this.#durationMs;
      if (beat.bar_position === 1) barNumber++;
      if (frac < startFrac || frac > endFrac) continue;

      const x = this.#xForProgress(w, frac, startFrac, endFrac);

      if (beat.bar_position === 1) {
        ctx.strokeStyle = 'rgba(255, 255, 255, 0.5)';
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(x, 0);
        ctx.lineTo(x, h);
        ctx.stroke();
        ctx.fillStyle = 'rgba(255, 255, 255, 0.6)';
        ctx.font = '9px system-ui, sans-serif';
        ctx.fillText(String(barNumber), x + 3, 10);
      } else {
        ctx.strokeStyle = 'rgba(255, 255, 255, 0.15)';
        ctx.lineWidth = 0.5;
        ctx.beginPath();
        ctx.moveTo(x, 0);
        ctx.lineTo(x, h);
        ctx.stroke();
      }
    }
  }

  #renderTimeGrid(
    ctx: CanvasRenderingContext2D,
    w: number,
    h: number,
    startFrac: number,
    endFrac: number,
  ) {
    if (!this.#durationMs) return;

    const visibleMs = (endFrac - startFrac) * this.#durationMs;
    let intervalMs: number;
    if (visibleMs > 120_000) intervalMs = 30_000;
    else if (visibleMs > 60_000) intervalMs = 10_000;
    else if (visibleMs > 20_000) intervalMs = 5_000;
    else if (visibleMs > 8_000) intervalMs = 2_000;
    else intervalMs = 1_000;

    const startMs = startFrac * this.#durationMs;
    const endMs = endFrac * this.#durationMs;
    const firstTick = Math.ceil(startMs / intervalMs) * intervalMs;

    ctx.fillStyle = 'rgba(255, 255, 255, 0.35)';
    ctx.font = '9px system-ui, sans-serif';

    for (let ms = firstTick; ms <= endMs; ms += intervalMs) {
      const frac = ms / this.#durationMs;
      const x = this.#xForProgress(w, frac, startFrac, endFrac);
      const secs = Math.floor(ms / 1000);
      const label = `${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, '0')}`;
      ctx.fillText(label, x + 2, h - 3);
    }
  }

  #renderCues(
    ctx: CanvasRenderingContext2D,
    w: number,
    h: number,
    startFrac: number,
    endFrac: number,
  ) {
    for (const cue of this.#cues) {
      const frac = Math.max(0, Math.min(1, cue.position ?? 0));
      if (frac < startFrac || frac > endFrac) continue;
      const x = this.#xForProgress(w, frac, startFrac, endFrac);
      const color = cue.color || '#4ade80';
      ctx.fillStyle = color;
      ctx.fillRect(x - 0.5, 0, 1, h);
      ctx.beginPath();
      ctx.arc(x, 2.5, 2.5, 0, Math.PI * 2);
      ctx.fill();
    }
  }

  #renderPlayhead(
    ctx: CanvasRenderingContext2D,
    w: number,
    h: number,
    startFrac: number,
    endFrac: number,
  ) {
    if (this.#progress < startFrac || this.#progress > endFrac) return;
    const x = this.#xForProgress(w, this.#progress, startFrac, endFrac);
    ctx.fillStyle =
      getComputedStyle(document.documentElement).getPropertyValue('--ff-accent').trim() || '#4a9eff';
    ctx.fillRect(x - 0.5, 0, 1, h);
  }

  #onPointerDown = (e: PointerEvent) => {
    if (e.button !== 0) return;
    this.#pointerDown = true;
    this.#gestureActive = false;
    this.#dragStartX = e.clientX;
    this.#dragStartY = e.clientY;
    this.#dragStartZoom = this.#zoom;
    this.#dragStartOffset = this.#offset;
    this.#canvas.setPointerCapture(e.pointerId);
    this.#canvas.style.cursor = 'grabbing';
    e.preventDefault();
  };

  #onPointerMove = (e: PointerEvent) => {
    if (!this.#pointerDown) return;

    const dx = e.clientX - this.#dragStartX;
    const dy = this.#dragStartY - e.clientY;

    if (!this.#gestureActive) {
      if (Math.hypot(dx, dy) < DRAG_THRESHOLD_PX) return;
      this.#gestureActive = true;
    }

    const newZoom = Math.max(
      ZOOM_MIN,
      Math.min(ZOOM_MAX, this.#dragStartZoom * Math.pow(1.015, dy)),
    );
    this.#zoom = newZoom;
    if (this.#zoom <= 1) {
      this.#offset = 0;
    } else {
      const rect = this.#canvas.getBoundingClientRect();
      const panAmount = (dx / rect.width) / this.#zoom;
      this.#offset = this.#clampOffset(this.#dragStartOffset - panAmount);
    }
    this.draw();
  };

  #onPointerUp = (e: PointerEvent) => {
    if (!this.#pointerDown) return;
    this.#pointerDown = false;
    this.#canvas.style.cursor = 'pointer';
    try {
      this.#canvas.releasePointerCapture(e.pointerId);
    } catch {
      /* already released */
    }

    if (!this.#gestureActive && this.#onScrub) {
      this.#onScrub(this.#progressAtClientX(e.clientX));
    }
    this.#gestureActive = false;
  };

  #onWheel = (e: WheelEvent) => {
    e.preventDefault();
    const anchor = this.#progressAtClientX(e.clientX);

    if (e.shiftKey || (this.#zoom > 1 && !e.altKey)) {
      const delta = (e.deltaX + e.deltaY) * 0.005;
      if (this.#zoom > 1) {
        this.#offset = this.#clampOffset(this.#offset + delta / this.#zoom);
        this.draw();
      }
      return;
    }

    const factor = e.deltaY < 0 ? 1.12 : 1 / 1.12;
    this.#setZoom(this.#zoom * factor, anchor);
  };

  #bindEvents() {
    this.#canvas.style.touchAction = 'none';
    this.#canvas.addEventListener('pointerdown', this.#onPointerDown);
    this.#canvas.addEventListener('wheel', this.#onWheel, { passive: false });
    window.addEventListener('pointermove', this.#onPointerMove);
    window.addEventListener('pointerup', this.#onPointerUp);
    window.addEventListener('pointercancel', this.#onPointerUp);
  }
}
