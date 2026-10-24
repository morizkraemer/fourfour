/**
 * WaveformDisplay — Lexicon-style DJ waveform component.
 *
 * Self-contained: no external dependencies, no Tauri, no global state.
 * Append to any container element. Call setData() with analysis data.
 *
 * Data shape expected by setData():
 *   {
 *     bands:         {low, mid, high} per-column 0..1 amplitudes (waveform-bands.js),
 *                    full track; the zoom view and the compact strip draw this,
 *     bandsOverview: optional lower-resolution {low, mid, high} for the overview bar,
 *     beats:         Array<{time_ms: number, bar_position: number}>,
 *     cues:          Array<{position: number, color?: string}>,  // position 0-1
 *     duration_ms:   number,
 *     bpm:           number,
 *   }
 *
 * Two modes (constructor option `mode`, default 'expanded'):
 *   'compact'  — single canvas, full-track static view, no zoom/pan, click/drag
 *                scrubs to seek. Playhead travels across the whole track.
 *   'expanded' — overview bar + zoom view. Overview: click/drag scrubs like
 *                compact (playhead jumps on the full-track preview). Zoom:
 *                Rekordbox-style playhead locked at centre while the waveform
 *                scrolls; horizontal drag scrubs, wheel zooms.
 */
import { drawBandRange, dimPlayed, ink, paletteVersion, wavePalette } from './waveform-render.js';


const DRAG_THRESHOLD_PX = 4;

// Playhead render delay (ms). The transport reports position at ~20Hz (~50ms).
// We display the playhead this far in the past so the render time always falls
// *between* two buffered ticks — pure interpolation gives constant-velocity
// scrolling, and rendering behind (never ahead of) the true position avoids a
// backward snap on pause. ~1.5 tick intervals: enough headroom to absorb tick
// jitter while staying imperceptible.
const PLAYHEAD_RENDER_DELAY_MS = 80;
// Recent authoritative ticks kept for interpolation. A handful is plenty to
// bracket the (delayed) render time even with jittery tick spacing.
const PLAYHEAD_TICK_BUFFER = 8;
// Bound on extrapolation past the newest tick, in multiples of the last gap —
// lets the head keep gliding through a dropped tick without running away.
const PLAYHEAD_MAX_EXTRAP = 2.0;

const OVERVIEW_DECAY = 0.93;
const COMPACT_DECAY = 0.88;
const ZOOM_DECAY = 0.86;

export default class WaveformDisplay {
    /**
     * @param {HTMLElement} container
     * @param {{ mode?: 'compact' | 'expanded', onSeek?: (frac: number) => void }} [options]
     */
    constructor(container, options = {}) {
        this._container = container;
        this._mode = options.mode === 'compact' ? 'compact' : 'expanded';
        // Expanded view locks the playhead at centre and scrolls the waveform.
        this._lockPlayhead = this._mode === 'expanded';
        if (options.onSeek) this.onSeek = options.onSeek;

        // Create wrapper div
        this._wrapper = document.createElement('div');
        this._wrapper.className = 'waveform-display';

        // Overview canvas (full-track mini-view) — wrapped for playhead overlay
        this._overviewCanvas = document.createElement('canvas');
        this._overviewCanvas.className = 'waveform-display__overview';
        const overviewWrap = document.createElement('div');
        overviewWrap.style.cssText = 'position:relative;flex-shrink:0;';
        if (this._mode === 'compact') overviewWrap.style.display = 'none';
        this._overviewWrap = overviewWrap;
        overviewWrap.appendChild(this._overviewCanvas);
        this._overviewPlayhead = document.createElement('div');
        this._overviewPlayhead.style.cssText =
            'position:absolute;top:0;bottom:0;left:0;width:1px;background:var(--ff-wave-playhead,#fff);pointer-events:none;display:none';
        overviewWrap.appendChild(this._overviewPlayhead);

        // Zoom canvas (scrollable detail view) — wrapped for playhead overlay
        this._zoomCanvas = document.createElement('canvas');
        this._zoomCanvas.className = 'waveform-display__zoom';
        const zoomWrap = document.createElement('div');
        zoomWrap.style.cssText = 'position:relative;flex:1;min-height:0;';
        zoomWrap.appendChild(this._zoomCanvas);
        this._zoomPlayhead = document.createElement('div');
        this._zoomPlayhead.style.cssText =
            'position:absolute;top:0;bottom:0;left:0;width:1px;background:var(--ff-wave-playhead,#fff);pointer-events:none;display:none;z-index:2';
        zoomWrap.appendChild(this._zoomPlayhead);
        this._playedMask = document.createElement('div');
        this._playedMask.style.cssText =
            'position:absolute;top:0;bottom:0;left:0;width:100%;background:var(--ff-wave-dim);pointer-events:none;display:none';
        zoomWrap.appendChild(this._playedMask);

        this._wrapper.appendChild(overviewWrap);
        this._wrapper.appendChild(zoomWrap);
        container.appendChild(this._wrapper);

        this._paletteVersion = paletteVersion();
        this._themeObserver = new MutationObserver(() => {
            this._overviewDirty = true;
            this._render();
        });
        this._themeObserver.observe(document.documentElement, {
            attributes: true,
            attributeFilter: ['class', 'style', 'data-theme'],
        });

        this._sizes = new WeakMap();
        this._sizeObserver = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver((entries) => {
            for (const entry of entries) {
                const box = entry.borderBoxSize && entry.borderBoxSize[0];
                this._sizes.set(entry.target, {
                    w: box ? box.inlineSize : entry.contentRect.width,
                    h: box ? box.blockSize : entry.contentRect.height,
                });
            }
            this._render();
        });
        if (this._sizeObserver) {
            this._sizeObserver.observe(this._overviewCanvas);
            this._sizeObserver.observe(this._zoomCanvas);
        }
        this._playheadX = new WeakMap();

        // Offscreen cache of the overview waveform (bars + cues). The overview
        // image never changes during playback — only the viewport rectangle
        // scrolls — so we render the bars once and blit them every frame.
        this._overviewCache = document.createElement('canvas');
        this._overviewDirty = true;
        this._overviewCacheW = 0;
        this._overviewCacheH = 0;

        // State
        this._data = null;
        this._zoom = 1.0;
        this._offset = 0.0;
        this._dragging = false;
        this._dragStartX = 0;
        this._dragStartY = 0;
        this._dragStartOffset = 0;
        this._dragStartZoom = 1.0;
        this._dragStartPlayhead = 0;
        this._dragMoved = false;
        // 'none' until the drag passes the threshold, then 'scrub' | 'zoom'.
        this._gesture = 'none';
        this._scrubbing = false;
        // 'overview' | 'zoom' — which surface started the current drag.
        this._dragSurface = null;
        this._playheadFrac = -1;

        // Playhead interpolation. The transport only reports position at ~20Hz,
        // which makes the locked-centre scroll visibly step. While playing we
        // free-run a rAF clock and render the playhead a fixed delay in the past,
        // linearly interpolating between the two most recent authoritative ticks.
        // Even tick spacing → constant-velocity scroll at the display refresh
        // rate (the thing that actually reads as smooth).
        this._playing = false;
        this._phTarget = -1;     // last authoritative position (0–1)
        this._phTargetAt = 0;    // timestamp of that report
        this._phClock = -1;      // smoothed, displayed position
        this._interpRaf = null;
        // Ring of recent authoritative ticks ({f: frac, t: timestamp}) the
        // render-delayed playhead interpolates across.
        this._samples = [];

        // rAF handle — coalesces rapid scroll/drag events into one draw per frame
        this._zoomRafId = null;

        this._bindEvents();
    }

    /** Switch render mode ('compact' | 'expanded'). */
    setMode(mode) {
        const next = mode === 'compact' ? 'compact' : 'expanded';
        if (next === this._mode) return;
        this._mode = next;
        this._lockPlayhead = next === 'expanded';
        this._overviewWrap.style.display = next === 'compact' ? 'none' : '';
        if (next === 'compact') {
            this._zoom = 1.0;
            this._offset = 0.0;
        }
        this._render();
        this._updatePlayheadOverlay();
    }

    /** Replace data and re-render. Resets zoom and scroll position. */
    setData(data, playheadFrac = undefined) {
        this._data = data;
        this._overviewDirty = true;
        this._resetSamples();
        if (playheadFrac !== undefined && playheadFrac >= 0) {
            this._playheadFrac = playheadFrac;
            this._phClock = playheadFrac;
            this._phTarget = playheadFrac;
            this._phTargetAt = this._now();
        } else {
            this._phClock = -1;
        }
        // Expanded opens pre-zoomed to ~5s on screen; compact shows the whole
        // track. Fit (resetZoom) returns expanded to the full-track view.
        this._zoom = this._defaultZoom();
        if (this._lockPlayhead) {
            this._offset = this._centeredOffset();
        } else {
            this._offset = 0.0;
        }
        this._paint();
    }

    // Initial zoom on load. Compact is always full-track. Expanded targets a
    // ~5s visible window (Rekordbox-style close-up) derived from the duration.
    _defaultZoom() {
        if (this._mode === 'compact') return 1.0;
        const dur = this._data?.duration_ms;
        if (!dur) return 1.0;
        const TARGET_VISIBLE_MS = 5000;
        return Math.max(1.0, Math.min(512.0, dur / TARGET_VISIBLE_MS));
    }

/** Clear to empty state. */
    clear() {
        this._data = null;
        this._overviewDirty = true;
        this._zoom = 1.0;
        this._offset = 0.0;
        this._render();
    }

    // Offset that keeps the current playhead centred in the zoom view.
    // At zoom 1 the whole track is visible, so show [0,1] and let the playhead
    // travel freely. When zoomed in, allow the offset to run past
    // [0, 1-visible] so the head stays dead-centre at the track ends (blank
    // space shows beyond the edges, Rekordbox-style).
    _centeredOffset() {
        if (this._zoom <= 1.0) return 0;
        const frac = this._playheadFrac < 0 ? 0 : this._playheadFrac;
        return frac - 0.5 / this._zoom;
    }

    /** Re-render at current size — call after container resize. */
    redraw() {
        this._render();
    }

    /**
     * Report the authoritative playhead position (0–1 fraction). Pass -1 to
     * hide. Fed by the (coarse, ~20Hz) transport — while playing the position
     * is smoothly interpolated up to the display refresh rate.
     */
    setPlayhead(frac) {
        this.setTransport(this._playing, frac);
    }

    /** Tell the engine whether the transport is playing (drives interpolation). */
    setPlaying(playing) {
        const frac = this._phTarget >= 0
            ? this._phTarget
            : (this._playheadFrac >= 0 ? this._playheadFrac : 0);
        this.setTransport(playing, frac);
    }

    /**
     * Atomic play-state + position update. Prefer this over separate setPlaying /
     * setPlayhead so play/pause transitions don't fight each other across effects.
     */
    setTransport(playing, frac) {
        if (this._scrubbing) return;
        const wasPlaying = this._playing;
        this._playing = !!playing;

        if (frac < 0) {
            this._phTarget = -1;
            this._stopInterp();
            this._resetSamples();
            this._phClock = -1;
            this._applyPlayhead(-1);
            return;
        }

        this._phTarget = frac;
        this._phTargetAt = this._now();

        if (!this._playing) {
            this._stopInterp();
            this._resetSamples();
            if (wasPlaying && this._phClock >= 0) {
                // Play→pause: freeze where the waveform is visually resting. While
                // playing we render ~RENDER_DELAY behind the true position, so
                // snapping to frac here would jump the waveform that far forward.
                // (Keep _phClock as-is.)
            } else {
                // Already paused — a position change is a deliberate seek; honor it.
                this._phClock = frac;
            }
            this._applyPlayhead(this._phClock);
            return;
        }

        // Resuming from pause / first tick — seed the interpolation buffer so it
        // continues from the frozen displayed position instead of snapping to the
        // authoritative frac (which sits ~RENDER_DELAY ahead of the frozen head).
        if (!wasPlaying || this._phClock < 0) {
            this._resetSamples();
            if (this._phClock >= 0) {
                // Place the frozen position one render-delay in the past so the
                // delayed interpolation picks up exactly there — no forward jump.
                this._samples.push({ f: this._phClock, t: this._now() - PLAYHEAD_RENDER_DELAY_MS });
            } else {
                this._phClock = frac;
            }
        }
        this._pushSample(frac);
        this._startInterp();
    }

    // Record an authoritative tick. Dedupe identical positions so a repeated
    // report doesn't flatten the interpolation slope (then jump on the next one).
    _pushSample(frac) {
        const s = this._samples;
        if (s.length && Math.abs(frac - s[s.length - 1].f) < 1e-9) return;
        s.push({ f: frac, t: this._now() });
        if (s.length > PLAYHEAD_TICK_BUFFER) s.shift();
    }

    _resetSamples() {
        this._samples = [];
    }

    _now() {
        return (typeof performance !== 'undefined' && performance.now)
            ? performance.now() : Date.now();
    }

    // Push a position to the canvas: in expanded the waveform scrolls under the
    // fixed centre playhead; in compact only the overlay element moves.
    _applyPlayhead(frac) {
        this._playheadFrac = frac;
        if (this._lockPlayhead && frac >= 0) {
            this._offset = this._centeredOffset();
            this._paint();
        } else {
            this._updatePlayheadOverlay();
        }
    }

    _startInterp() {
        if (this._interpRaf || !this._playing) return;
        const loop = () => {
            if (!this._playing) { this._interpRaf = null; return; }
            this._interpRaf = requestAnimationFrame(loop);
            this._stepInterp();
        };
        this._interpRaf = requestAnimationFrame(loop);
    }

    _stopInterp() {
        if (this._interpRaf) cancelAnimationFrame(this._interpRaf);
        this._interpRaf = null;
    }

    // Render the playhead a fixed delay in the past, linearly interpolating
    // between the two buffered ticks that bracket the render time. Because the
    // render time stays between known ticks, the head moves at constant velocity
    // (steady tick spacing → zero per-frame velocity variance), which is what
    // reads as smooth. Past the newest tick we extrapolate along the last
    // segment's slope (bounded) so a dropped tick doesn't freeze the scroll.
    _stepInterp() {
        if (this._scrubbing) return;
        const s = this._samples;
        if (s.length === 0) return;
        if (s.length === 1) {
            this._setClock(s[0].f);
            return;
        }

        const renderT = this._now() - PLAYHEAD_RENDER_DELAY_MS;
        // Bracketing pair: last segment whose start is at/before renderT.
        let i = s.length - 2;
        for (let k = 0; k < s.length - 1; k++) {
            if (s[k + 1].t > renderT) { i = k; break; }
        }
        const a = s[i], b = s[i + 1];
        const span = b.t - a.t;
        const u = span > 0
            ? Math.max(0, Math.min(PLAYHEAD_MAX_EXTRAP, (renderT - a.t) / span))
            : 1;
        this._setClock(a.f + (b.f - a.f) * u);
    }

    _setClock(pos) {
        pos = Math.max(0, Math.min(1, pos));
        if (Math.abs(pos - this._phClock) < 1e-9) return;
        this._phClock = pos;
        this._applyPlayhead(this._phClock);
    }

    _canvasSize(canvas) {
        const cached = this._sizes.get(canvas);
        if (cached) return cached;
        const rect = canvas.getBoundingClientRect();
        return { w: rect.width, h: rect.height };
    }

    _showPlayhead(el, frac, canvas) {
        if (el.style.display !== 'block') el.style.display = 'block';
        const x = Math.round(frac * this._canvasSize(canvas).w * 64) / 64 - 0.5;
        if (this._playheadX.get(el) === x) return;
        this._playheadX.set(el, x);
        el.style.transform = `translateX(${x}px)`;
    }

    _showPlayedMask(frac) {
        const mask = this._playedMask;
        if (mask.style.display !== 'block') mask.style.display = 'block';
        const w = this._canvasSize(this._zoomCanvas).w;
        const x = Math.round(frac * w * 64) / 64 - w;
        if (this._playheadX.get(mask) === x) return;
        this._playheadX.set(mask, x);
        mask.style.transform = `translateX(${x}px)`;
    }

    _hidePlayhead(el) {
        if (el.style.display !== 'none') el.style.display = 'none';
    }

    _updatePlayheadOverlay() {
        const frac = this._playheadFrac;
        if (frac < 0 || frac > 1) {
            this._hidePlayhead(this._overviewPlayhead);
            this._hidePlayhead(this._zoomPlayhead);
            this._hidePlayhead(this._playedMask);
            return;
        }
        if (this._mode !== 'compact') {
            this._showPlayhead(this._overviewPlayhead, frac, this._overviewCanvas);
            this._hidePlayhead(this._playedMask);
        } else {
            this._showPlayedMask(frac);
        }
        if (this._lockPlayhead) {
            this._hidePlayhead(this._zoomPlayhead);
            return;
        }
        const visibleFrac = 1.0 / this._zoom;
        const start = this._offset;
        const end = start + visibleFrac;
        if (frac >= start && frac <= end) {
            this._showPlayhead(this._zoomPlayhead, (frac - start) / visibleFrac, this._zoomCanvas);
        } else {
            this._hidePlayhead(this._zoomPlayhead);
        }
    }

    /** Programmatically set zoom and scroll position (for syncing displays). */
    setViewport(zoom, offset) {
        this._zoom = Math.max(1.0, Math.min(512.0, zoom));
        this._offset = this._clampOffset(offset);
        this._render();
        this._updatePlayheadOverlay();
    }

    // ── Zoom controls (expanded toolbar) ───────────────────────────────────
    // Each keeps the playhead centred in expanded mode. No-ops in compact.

    /** Set zoom, re-centring on the playhead (expanded) or clamping (free). */
    _applyZoom(next) {
        if (this._mode === 'compact') return;
        this._zoom = Math.max(1.0, Math.min(512.0, next));
        this._offset = this._lockPlayhead ? this._centeredOffset() : this._clampOffset(this._offset);
        this._render();
        this._updatePlayheadOverlay();
        if (this.onViewportChange) this.onViewportChange(this._zoom, this._offset);
    }

    zoomIn()    { this._applyZoom(this._zoom * 1.5); }
    zoomOut()   { this._applyZoom(this._zoom / 1.5); }
    resetZoom() { this._applyZoom(1.0); }

    /** Remove canvases and all event listeners. */
    destroy() {
        if (this._zoomRafId) cancelAnimationFrame(this._zoomRafId);
        this._stopInterp();
        if (this._sizeObserver) this._sizeObserver.disconnect();
        this._themeObserver.disconnect();
        this._unbindEvents();
        this._wrapper.remove();
    }

    // ── Private ──────────────────────────────────────────────────────────

    // Synchronous full repaint of both canvases + overlays. Called directly from
    // the interpolation loop (already inside a frame); use _render() elsewhere.
    _paint() {
        this._renderOverview();
        this._renderZoom();
        this._updatePlayheadOverlay();
    }

    _render() {
        // Coalesce rapid playhead/scroll/drag updates into one draw per frame.
        if (this._zoomRafId) cancelAnimationFrame(this._zoomRafId);
        this._zoomRafId = requestAnimationFrame(() => {
            this._zoomRafId = null;
            this._paint();
        });
    }

    // Resize a canvas only when its pixel dimensions actually change.
    // Setting canvas.width always clears to transparent — that clear-then-draw
    // gap is what causes visible flicker on every scroll event.
    _resizeCanvas(canvas, w, h) {
        const dprW = Math.round(w * devicePixelRatio);
        const dprH = Math.round(h * devicePixelRatio);
        if (canvas.width !== dprW || canvas.height !== dprH) {
            canvas.width = dprW;
            canvas.height = dprH;
        }
        const ctx = canvas.getContext('2d');
        // Reset transform each frame (setTransform doesn't require a canvas reset)
        ctx.setTransform(devicePixelRatio, 0, 0, devicePixelRatio, 0, 0);
        return ctx;
    }

    // ── Overview ─────────────────────────────────────────────────────────

    _renderOverview() {
        if (this._mode === 'compact') return;
        const canvas = this._overviewCanvas;
        const size = this._canvasSize(canvas);
        if (size.w === 0) return;

        const w = size.w;
        const h = size.h;
        const ctx = this._resizeCanvas(canvas, w, h);
        const palette = wavePalette();

        if (this._overviewDirty || this._overviewCacheW !== w || this._overviewCacheH !== h || this._paletteVersion !== palette.version) {
            this._paletteVersion = palette.version;
            this._renderOverviewCache(w, h, palette);
            this._overviewCacheW = w;
            this._overviewCacheH = h;
            this._overviewDirty = false;
        }

        ctx.drawImage(this._overviewCache, 0, 0, w, h);

        if (this._playheadFrac > 0) {
            dimPlayed(ctx, Math.min(1, this._playheadFrac) * w, h, palette);
        }

        const visibleFrac = 1.0 / this._zoom;
        const x1 = Math.max(0, this._offset * w);
        const x2 = Math.min(w, (this._offset + visibleFrac) * w);
        if (this._zoom > 1.0 && x2 > x1) {
            ctx.fillStyle = ink(palette, 0.08);
            ctx.fillRect(x1, 0, x2 - x1, h);
            ctx.strokeStyle = ink(palette, 0.3);
            ctx.lineWidth = 1;
            ctx.strokeRect(x1, 0, x2 - x1, h);
        }
    }

    _renderOverviewCache(w, h, palette) {
        const cache = this._overviewCache;
        const dpr = devicePixelRatio;
        const widthPx = Math.round(w * dpr);
        const heightPx = Math.round(h * dpr);
        if (cache.width !== widthPx || cache.height !== heightPx) {
            cache.width = widthPx;
            cache.height = heightPx;
        }
        const ctx = cache.getContext('2d');
        ctx.setTransform(1, 0, 0, 1, 0, 0);
        ctx.fillStyle = palette.bg;
        ctx.fillRect(0, 0, widthPx, heightPx);

        const bands = this._data?.bandsOverview || this._data?.bands;
        if (bands && bands.low.length > 0) {
            drawBandRange(ctx, bands, {
                startFrac: 0, endFrac: 1, widthPx, heightPx, palette, dpr, decay: OVERVIEW_DECAY,
            });
        }

        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        const cues = this._data?.cues;
        if (cues) {
            for (const cue of cues) {
                const frac = Math.max(0, Math.min(1, cue.position ?? 0));
                ctx.fillStyle = cue.color || '#4ade80';
                ctx.fillRect(frac * w - 0.5, 0, 1, h);
            }
        }
    }

    // ── Zoom ──────────────────────────────────────────────────────────────

    _renderZoom() {
        const canvas = this._zoomCanvas;
        const size = this._canvasSize(canvas);
        if (size.w === 0) return;

        const w = size.w;
        const h = size.h;
        const ctx = this._resizeCanvas(canvas, w, h);
        const palette = wavePalette();

        ctx.fillStyle = palette.bg;
        ctx.fillRect(0, 0, w, h);

        if (!this._data) return;

        const visibleFrac = 1.0 / this._zoom;
        const startFrac = this._offset;
        const endFrac = startFrac + visibleFrac;

        this._renderBands(ctx, w, h, startFrac, endFrac, palette);
        if (this._lockPlayhead && this._playheadFrac > startFrac) {
            const playedW = Math.min(w, ((this._playheadFrac - startFrac) / visibleFrac) * w);
            dimPlayed(ctx, playedW, h, palette);
        }

        this._renderBeatGrid(ctx, w, h, startFrac, endFrac, palette);
        this._renderTimeGrid(ctx, w, h, startFrac, endFrac, palette);
        this._renderCues(ctx, w, h, startFrac, endFrac);
        this._renderCentrePlayhead(ctx, w, h, palette);
    }

    _renderCentrePlayhead(ctx, w, h, palette) {
        if (!this._lockPlayhead || this._playheadFrac < 0) return;
        const x = this._zoom <= 1.0 ? this._playheadFrac * w : w / 2;
        ctx.strokeStyle = palette.playhead;
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(x, 0);
        ctx.lineTo(x, h);
        ctx.stroke();
    }

    // Hot-cue markers: a vertical line + a dot at the top, matching the
    // compact strip so the two views agree. Positions are 0–1 fractions.
    _renderCues(ctx, w, h, startFrac, endFrac) {
        const cues = this._data?.cues;
        if (!cues || cues.length === 0 || endFrac <= startFrac) return;
        for (const cue of cues) {
            const frac = Math.max(0, Math.min(1, cue.position ?? 0));
            if (frac < startFrac || frac > endFrac) continue;
            const x = ((frac - startFrac) / (endFrac - startFrac)) * w;
            const color = cue.color || '#4ade80';
            ctx.fillStyle = color;
            ctx.fillRect(x - 0.5, 0, 1, h);
            ctx.beginPath();
            ctx.arc(x, 2.5, 2.5, 0, Math.PI * 2);
            ctx.fill();
        }
    }

    _renderBands(ctx, w, h, startFrac, endFrac, palette) {
        const bands = this._data.bands;
        if (!bands || bands.low.length === 0) return;
        const dpr = devicePixelRatio;
        ctx.save();
        ctx.setTransform(1, 0, 0, 1, 0, 0);
        drawBandRange(ctx, bands, {
            startFrac,
            endFrac,
            widthPx: Math.round(w * dpr),
            heightPx: Math.round(h * dpr),
            palette,
            dpr,
            decay: this._mode === 'compact' ? COMPACT_DECAY : ZOOM_DECAY,
            scrolling: true,
        });
        ctx.restore();
    }

    // How many CSS pixels each beat gets in the current zoom viewport.
    _pxPerBeat(w, startFrac, endFrac, beats, durationMs) {
        const span = endFrac - startFrac;
        if (span <= 0 || !beats.length) return Infinity;
        let count = 0;
        for (const beat of beats) {
            const frac = beat.time_ms / durationMs;
            if (frac >= startFrac && frac <= endFrac) count++;
        }
        return w / Math.max(1, count);
    }

    // Beat-grid density for expanded zoom — thins out as more track fits on screen.
    _beatGridDetail(pxPerBeat) {
        if (pxPerBeat < 5) return 'sections';   // every 16 bars
        if (pxPerBeat < 10) return 'phrases';   // every 4 bars
        if (pxPerBeat < 20) return 'bars';      // bar lines, no beat ticks
        if (pxPerBeat < 32) return 'bars+beats'; // bars + faint beat ticks
        return 'full';
    }

    _beatX(frac, startFrac, endFrac, w) {
        return ((frac - startFrac) / (endFrac - startFrac)) * w;
    }

    _labelWidth(ctx, label) {
        if (!this._labelWidths) this._labelWidths = new Map();
        const key = ctx.font + '|' + label;
        let width = this._labelWidths.get(key);
        if (width === undefined) {
            width = ctx.measureText(label).width;
            this._labelWidths.set(key, width);
        }
        return width;
    }

    _drawBarLabel(ctx, x, w, h, barNumber, isPhrase, palette) {
        const fontSize = 10;
        const label = String(barNumber);
        ctx.font = `${isPhrase ? 'bold ' : ''}${fontSize}px system-ui`;
        const labelW = this._labelWidth(ctx, label) + 2;
        const nearCentre = Math.abs(x - w / 2) < 28;
        ctx.fillStyle = palette.bg;
        if (nearCentre) {
            ctx.textAlign = 'left';
            ctx.fillRect(x + 2, 0, labelW + 2, fontSize + 4);
            ctx.fillStyle = ink(palette, isPhrase ? 0.9 : 0.6);
            ctx.fillText(label, x + 4, fontSize + 2);
        } else {
            ctx.textAlign = 'right';
            ctx.fillRect(x - labelW - 2, 0, labelW + 2, fontSize + 4);
            ctx.fillStyle = ink(palette, isPhrase ? 0.9 : 0.6);
            ctx.fillText(label, x - 2, fontSize + 2);
        }
    }

    _renderBeatGrid(ctx, w, h, startFrac, endFrac, palette) {
        const beats = this._data?.beats;
        const durationMs = this._data?.duration_ms;
        if (!beats || beats.length === 0 || !durationMs) return;

        const span = endFrac - startFrac;
        if (span <= 0) return;

        // Compact shows the whole track — phrase ticks only.
        if (this._mode === 'compact') {
            this._renderSparseBeatGrid(ctx, w, h, startFrac, endFrac, beats, durationMs, 4, false, palette);
            return;
        }

        const detail = this._beatGridDetail(
            this._pxPerBeat(w, startFrac, endFrac, beats, durationMs));

        if (detail === 'sections') {
            this._renderSparseBeatGrid(ctx, w, h, startFrac, endFrac, beats, durationMs, 16, true, palette);
            return;
        }
        if (detail === 'phrases') {
            this._renderSparseBeatGrid(ctx, w, h, startFrac, endFrac, beats, durationMs, 4, true, palette);
            return;
        }

        let barNumber = 0;
        const showBeats = detail === 'full' || detail === 'bars+beats';
        const labelBars = detail === 'full' || detail === 'bars' || detail === 'bars+beats';
        const labelPhrasesOnly = detail === 'bars' || detail === 'bars+beats';

        for (const beat of beats) {
            const frac = beat.time_ms / durationMs;
            if (beat.bar_position === 1) barNumber++;
            if (frac < startFrac || frac > endFrac) continue;

            const x = this._beatX(frac, startFrac, endFrac, w);
            const isBar = beat.bar_position === 1;
            const isPhrase = isBar && barNumber % 4 === 1;

            if (!isBar) {
                if (!showBeats) continue;
                ctx.strokeStyle = ink(palette, 0.3);
                ctx.lineWidth = 0.5;
                ctx.beginPath();
                ctx.moveTo(x, 0);
                ctx.lineTo(x, h);
                ctx.stroke();
            } else {
                ctx.strokeStyle = ink(palette, detail === 'bars+beats' ? 0.55 : 0.7);
                ctx.lineWidth = 1;
                ctx.beginPath();
                ctx.moveTo(x, 0);
                ctx.lineTo(x, h);
                ctx.stroke();

                if (labelBars && (!labelPhrasesOnly || isPhrase)) {
                    this._drawBarLabel(ctx, x, w, h, barNumber, isPhrase, palette);
                }
            }
        }
    }

    // Sparse bar grid — every `barStride` bars (4 = phrase, 16 = section).
    _renderSparseBeatGrid(ctx, w, h, startFrac, endFrac, beats, durationMs, barStride, labelPhrases, palette) {
        const span = endFrac - startFrac;
        if (span <= 0) return;
        let barNumber = 0;
        ctx.strokeStyle = ink(palette, barStride >= 16 ? 0.22 : 0.28);
        ctx.lineWidth = 1;
        for (const beat of beats) {
            if (beat.bar_position !== 1) continue;
            barNumber++;
            if (barNumber % barStride !== 1) continue;
            const frac = beat.time_ms / durationMs;
            if (frac < startFrac || frac > endFrac) continue;
            const x = this._beatX(frac, startFrac, endFrac, w);
            ctx.beginPath();
            ctx.moveTo(x, 0);
            ctx.lineTo(x, h);
            ctx.stroke();
            if (labelPhrases && barStride === 4) {
                this._drawBarLabel(ctx, x, w, h, barNumber, true, palette);
            }
        }
    }

    _renderTimeGrid(ctx, w, h, startFrac, endFrac, palette) {
        const durationMs = this._data?.duration_ms;
        if (!durationMs) return;

        const visibleMs = (endFrac - startFrac) * durationMs;
        const intervalMs =
            visibleMs > 120000 ? 30000 :
            visibleMs > 60000  ? 10000 :
            visibleMs > 20000  ? 5000  :
            visibleMs > 8000   ? 2000  : 1000;

        const startMs = startFrac * durationMs;
        const endMs = endFrac * durationMs;
        const firstTick = Math.ceil(startMs / intervalMs) * intervalMs;

        // Compact: tuck the labels right against the bottom edge, smaller and
        // dimmer, so they sit out of the way of the (now taller) waveform.
        const compact = this._mode === 'compact';
        ctx.fillStyle = ink(palette, compact ? 0.22 : 0.35);
        ctx.font = compact ? '8px system-ui' : '9px system-ui';
        ctx.textAlign = 'left';
        ctx.textBaseline = compact ? 'bottom' : 'alphabetic';
        const yLabel = compact ? h : h - 3;

        for (let ms = firstTick; ms <= endMs; ms += intervalMs) {
            const x = ((ms / durationMs - startFrac) / (endFrac - startFrac)) * w;
            const secs = Math.floor(ms / 1000);
            ctx.fillText(
                `${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, '0')}`,
                x + 2, yLabel,
            );
        }
        ctx.textBaseline = 'alphabetic';
    }

    // ── Interaction ───────────────────────────────────────────────────────

    _clampOffset(offset) {
        return Math.max(0, Math.min(1.0 - 1.0 / this._zoom, offset));
    }

    _defaultCursor() {
        return this._mode === 'compact' ? 'pointer' : 'default';
    }

    // Track fraction (0–1) under a clientX on the overview bar (full track).
    _overviewFracAtClientX(clientX) {
        const rect = this._overviewCanvas.getBoundingClientRect();
        if (rect.width <= 0) return 0;
        return Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
    }

    // Track fraction (0–1) under a clientX on the zoom canvas, honouring the
    // current viewport. In compact (zoom 1, offset 0) this is just x/width.
    _zoomFracAtClientX(clientX) {
        const rect = this._zoomCanvas.getBoundingClientRect();
        const clickFrac = (clientX - rect.left) / rect.width;
        const visibleFrac = 1.0 / this._zoom;
        return Math.max(0, Math.min(1, this._offset + clickFrac * visibleFrac));
    }

    // Apply a scrub position visually (seek is deferred to mouseup).
    _previewScrub(frac) {
        this._playheadFrac = Math.max(0, Math.min(1, frac));
        if (this._lockPlayhead) this._offset = this._centeredOffset();
        this._render();
        this._updatePlayheadOverlay();
    }

    _bindEvents() {
        this._onOverviewMouseDown = (e) => {
            if (e.button !== 0 || this._mode === 'compact') return;
            this._dragging = true;
            this._dragSurface = 'overview';
            this._dragMoved = false;
            this._gesture = 'none';
            this._scrubbing = false;
            this._dragStartX = e.clientX;
            this._dragStartY = e.clientY;
            e.preventDefault();
        };

        this._onMouseDown = (e) => {
            if (e.button !== 0) return;
            this._dragging = true;
            this._dragSurface = 'zoom';
            this._dragMoved = false;
            this._gesture = 'none';
            this._scrubbing = false;
            this._dragStartX = e.clientX;
            this._dragStartY = e.clientY;
            this._dragStartOffset = this._offset;
            this._dragStartZoom = this._zoom;
            this._dragStartPlayhead = this._playheadFrac < 0 ? 0 : this._playheadFrac;
            e.preventDefault();
        };

        this._onMouseMove = (e) => {
            if (!this._dragging) return;
            const dx = e.clientX - this._dragStartX;
            const dy = this._dragStartY - e.clientY;

            // Dragging always scrubs (both modes). Zoom is button/wheel only.
            if (this._gesture === 'none') {
                if (Math.hypot(dx, dy) < DRAG_THRESHOLD_PX) return;
                this._dragMoved = true;
                this._gesture = 'scrub';
                this._scrubbing = true;
                const cursorEl = this._dragSurface === 'overview'
                    ? this._overviewCanvas
                    : this._zoomCanvas;
                cursorEl.style.cursor = 'ew-resize';
            }

            // Visual-only: move the playhead now, defer the seek to mouseup so we
            // don't flood the audio backend.
            let frac;
            if (this._dragSurface === 'overview') {
                frac = this._overviewFracAtClientX(e.clientX);
            } else if (this._lockPlayhead) {
                const rect = this._zoomCanvas.getBoundingClientRect();
                const visibleFrac = 1.0 / this._zoom;
                frac = this._dragStartPlayhead - (dx / rect.width) * visibleFrac;
            } else {
                frac = this._zoomFracAtClientX(e.clientX);
            }
            this._previewScrub(frac);
        };

        this._onMouseUp = (e) => {
            if (!this._dragging) return;
            const surface = this._dragSurface;
            this._dragging = false;
            this._dragSurface = null;

            if (surface === 'overview') {
                this._overviewCanvas.style.cursor = 'pointer';
            } else {
                this._zoomCanvas.style.cursor = this._defaultCursor();
            }

            // Scrub drag or a plain click — issue the actual seek here.
            let frac;
            if (this._gesture === 'scrub') {
                frac = this._playheadFrac;
            } else if (surface === 'overview') {
                frac = this._overviewFracAtClientX(e.clientX);
            } else {
                frac = this._zoomFracAtClientX(e.clientX);
            }
            this._gesture = 'none';
            this._scrubbing = false;
            if (this.onSeek && this._data?.duration_ms) {
                this._playheadFrac = Math.max(0, Math.min(1, frac));
                this._phClock = this._playheadFrac;
                this._phTarget = this._playheadFrac;
                this._phTargetAt = this._now();
                this._resetSamples();
                if (this._lockPlayhead) this._offset = this._centeredOffset();
                this._render();
                this._updatePlayheadOverlay();
                this.onSeek(this._playheadFrac);
            }
        };

        // Wheel zooms in expanded (centred); compact ignores it.
        this._onWheel = (e) => {
            if (this._mode === 'compact') return;
            e.preventDefault();
            const factor = e.deltaY < 0 ? 1.1 : 1 / 1.1;
            this._applyZoom(this._zoom * factor);
        };

        this._zoomCanvas.style.cursor = this._defaultCursor();
        this._overviewCanvas.addEventListener('mousedown', this._onOverviewMouseDown);
        this._zoomCanvas.addEventListener('mousedown', this._onMouseDown);
        window.addEventListener('mousemove', this._onMouseMove);
        window.addEventListener('mouseup', this._onMouseUp);
        this._zoomCanvas.addEventListener('wheel', this._onWheel, { passive: false });
    }

    _unbindEvents() {
        this._overviewCanvas.removeEventListener('mousedown', this._onOverviewMouseDown);
        this._zoomCanvas.removeEventListener('mousedown', this._onMouseDown);
        window.removeEventListener('mousemove', this._onMouseMove);
        window.removeEventListener('mouseup', this._onMouseUp);
        this._zoomCanvas.removeEventListener('wheel', this._onWheel);
    }
}
