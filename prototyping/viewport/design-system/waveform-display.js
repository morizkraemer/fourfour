/**
 * WaveformDisplay — Lexicon-style DJ waveform component.
 *
 * Self-contained: no external dependencies, no Tauri, no global state.
 * Append to any container element. Call setData() with analysis data.
 *
 * Data shape expected by setData():
 *   {
 *     waveform_color:   Array<{amp: number, r: number, g: number, b: number}>,
 *     waveform_preview: Uint8Array | number[],  // 400-byte Pioneer PWAV fallback
 *     beats:            Array<{time_ms: number, bar_position: number}>,
 *     cues:             Array<{position: number, color?: string}>,  // position 0–1
 *     duration_ms:      number,
 *     bpm:              number,
 *   }
 *
 * Two modes (constructor option `mode`, default 'expanded'):
 *   'compact'  — single canvas, full-track static view, no zoom/pan, click/drag
 *                scrubs to seek. Playhead travels across the whole track.
 *   'expanded' — overview bar + zoom view, Rekordbox-style playhead locked at
 *                centre while the waveform scrolls. Horizontal drag scrubs,
 *                vertical drag / wheel zooms (playhead stays centred).
 */

const FALLBACK_COLOR = 'rgb(80, 80, 80)';
const WAVEFORM_ALPHA = 0.85;
const DRAG_THRESHOLD_PX = 4;

// Rekordbox 3-band colors: bass=blue, mids=orange, highs=white
const COLOR_BASS = 'rgb(30,  100, 255)';
const COLOR_MID  = 'rgb(255, 140,   0)';
const COLOR_HIGH = 'rgb(255, 255, 255)';

// Overview bar: softer than the zoom view so the full-track preview reads smooth.
const OVERVIEW_DECAY = 0.93;
const OVERVIEW_ATTACK = 0.4;

// amp: peak loudness 0–1 (1.0 = 0 dBFS). r/g/b: relative band weights 0–1.
function bandAmps(amp, bassW, midW, highW) {
    return [bassW * amp, midW * amp, highW * amp];
}

function lerpColumn(d0, d1, t) {
    return {
        amp: d0.amp + (d1.amp - d0.amp) * t,
        r: d0.r + (d1.r - d0.r) * t,
        g: d0.g + (d1.g - d0.g) * t,
        b: d0.b + (d1.b - d0.b) * t,
    };
}

const EMPTY_COLUMN = { amp: 0, r: 0, g: 0, b: 0 };

// Linear blend between waveform columns at a fractional track position.
// frac < 0 → blank (Rekordbox-style padding before track start).
function sampleColumnAtFrac(data, N, frac) {
    if (frac <= 0) return EMPTY_COLUMN;
    if (frac >= 1) return data[N - 1];
    const f = frac * N;
    if (f >= N - 1) return data[N - 1];
    const i0 = Math.floor(f);
    return lerpColumn(data[i0], data[i0 + 1], f - i0);
}

// Apply an envelope follower: zoom-invariant exponential decay.
// decayPerColumn controls how fast the tail falls after a transient — the same
// number of waveform columns regardless of current zoom level.
// attackCoeff 1 = instant attack; lower values soften rising transients.
function applyEnvelope(amps, samplesPerPixel, decayPerColumn = 0.82, attackCoeff = 1) {
    const out = new Float32Array(amps.length);
    // Convert column-space decay to pixel-space so it's zoom-invariant.
    // Zoomed out (spp > 1): many columns per pixel → fast pixel decay.
    // Zoomed in  (spp < 1): fraction of a column per pixel → slow pixel decay.
    const decayPerPx = Math.pow(decayPerColumn, Math.max(samplesPerPixel, 0.01));
    let env = 0;
    for (let i = 0; i < amps.length; i++) {
        if (amps[i] > env) {
            env = attackCoeff >= 1
                ? amps[i]
                : env + (amps[i] - env) * attackCoeff;
        } else {
            env *= decayPerPx;
            if (env < amps[i]) env = amps[i];
        }
        out[i] = env;
    }
    return out;
}

// Draw one waveform band as a filled symmetric shape (mirrored above/below centre).
// Filled paths look smooth and continuous; individual strokes look blocky.
function drawBand(ctx, amps, w, yCenter, scale, color) {
    ctx.fillStyle = color;
    ctx.beginPath();
    ctx.moveTo(0, yCenter);
    for (let px = 0; px < w; px++) {
        ctx.lineTo(px, yCenter - amps[px] * scale);
    }
    for (let px = w - 1; px >= 0; px--) {
        ctx.lineTo(px, yCenter + amps[px] * scale);
    }
    ctx.closePath();
    ctx.fill();
}


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
            'position:absolute;top:0;bottom:0;width:1px;background:#fff;pointer-events:none;display:none;transform:translateX(-50%)';
        overviewWrap.appendChild(this._overviewPlayhead);

        // Zoom canvas (scrollable detail view) — wrapped for playhead overlay
        this._zoomCanvas = document.createElement('canvas');
        this._zoomCanvas.className = 'waveform-display__zoom';
        const zoomWrap = document.createElement('div');
        zoomWrap.style.cssText = 'position:relative;flex:1;min-height:0;';
        zoomWrap.appendChild(this._zoomCanvas);
        this._zoomPlayhead = document.createElement('div');
        this._zoomPlayhead.style.cssText =
            'position:absolute;top:0;bottom:0;width:1px;background:#fff;pointer-events:none;display:none;transform:translateX(-50%);z-index:2';
        zoomWrap.appendChild(this._zoomPlayhead);

        this._wrapper.appendChild(overviewWrap);
        this._wrapper.appendChild(zoomWrap);
        container.appendChild(this._wrapper);

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
        this._playheadFrac = -1;

        // Playhead interpolation. The transport only reports position at ~20Hz,
        // which makes the locked-centre scroll visibly step. While playing we
        // free-run a rAF clock that predicts the position between ticks and
        // eases toward it, so the waveform scrolls at the display refresh rate.
        this._playing = false;
        this._phTarget = -1;     // last authoritative position (0–1)
        this._phTargetAt = 0;    // timestamp of that report
        this._phClock = -1;      // smoothed, displayed position
        this._interpRaf = null;

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
            this._phClock = -1;
            this._applyPlayhead(-1);
            return;
        }

        this._phTarget = frac;
        this._phTargetAt = this._now();

        if (!this._playing) {
            this._stopInterp();
            this._phClock = frac;
            this._applyPlayhead(frac);
            return;
        }

        // Resuming from pause — start from the reported position, not a stale clock.
        if (!wasPlaying || this._phClock < 0) {
            this._phClock = frac;
        }
        this._startInterp();
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

    // Ease the displayed clock toward the last transport tick. No forward
    // extrapolation — predicting ahead of ticks caused a backward snap on pause.
    _stepInterp() {
        if (this._scrubbing) return;
        if (this._phTarget < 0) return;
        const diff = this._phTarget - this._phClock;
        if (Math.abs(diff) > 0.05) {
            this._phClock = this._phTarget;
        } else if (Math.abs(diff) > 1e-6) {
            this._phClock += diff * 0.35;
        } else {
            return;
        }
        this._applyPlayhead(this._phClock);
    }

    _updatePlayheadOverlay() {
        const frac = this._playheadFrac;
        if (frac < 0 || frac > 1) {
            this._overviewPlayhead.style.display = 'none';
            this._zoomPlayhead.style.display = 'none';
            return;
        }
        if (this._mode !== 'compact') {
            this._overviewPlayhead.style.display = 'block';
            this._overviewPlayhead.style.left = (frac * 100) + '%';
        }
        if (this._lockPlayhead) {
            // Centre playhead is drawn on the zoom canvas (above the beat grid).
            this._zoomPlayhead.style.display = 'none';
            return;
        }
        const visibleFrac = 1.0 / this._zoom;
        const start = this._offset;
        const end = start + visibleFrac;
        if (frac >= start && frac <= end) {
            this._zoomPlayhead.style.display = 'block';
            this._zoomPlayhead.style.left = ((frac - start) / visibleFrac * 100) + '%';
        } else {
            this._zoomPlayhead.style.display = 'none';
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
        if (this._mode === 'compact') return; // overview hidden in compact
        const canvas = this._overviewCanvas;
        const rect = canvas.getBoundingClientRect();
        if (rect.width === 0) return;

        const w = rect.width;
        const h = rect.height;
        const ctx = this._resizeCanvas(canvas, w, h);

        // Re-render the (expensive) bars+cues into the offscreen cache only when
        // the data or the canvas size changed — not on every playback frame.
        if (this._overviewDirty || this._overviewCacheW !== w || this._overviewCacheH !== h) {
            this._renderOverviewCache(w, h);
            this._overviewCacheW = w;
            this._overviewCacheH = h;
            this._overviewDirty = false;
        }

        // Cheap per-frame path: blit the cached bars, then draw the moving
        // viewport indicator on top.
        ctx.drawImage(this._overviewCache, 0, 0, w, h);

        const visibleFrac = 1.0 / this._zoom;
        const x1 = Math.max(0, this._offset * w);
        const x2 = Math.min(w, (this._offset + visibleFrac) * w);
        if (this._zoom > 1.0 && x2 > x1) {
            ctx.fillStyle = 'rgba(255, 255, 255, 0.08)';
            ctx.fillRect(x1, 0, x2 - x1, h);
            ctx.strokeStyle = 'rgba(255, 255, 255, 0.3)';
            ctx.lineWidth = 1;
            ctx.strokeRect(x1, 0, x2 - x1, h);
        }
    }

    // Render the static overview (background + 3-band bars + cue ticks) into the
    // offscreen cache canvas at the given CSS size.
    _renderOverviewCache(w, h) {
        const cache = this._overviewCache;
        const dprW = Math.round(w * devicePixelRatio);
        const dprH = Math.round(h * devicePixelRatio);
        if (cache.width !== dprW || cache.height !== dprH) {
            cache.width = dprW;
            cache.height = dprH;
        }
        const ctx = cache.getContext('2d');
        ctx.setTransform(devicePixelRatio, 0, 0, devicePixelRatio, 0, 0);

        ctx.fillStyle = '#0d0d0d';
        ctx.fillRect(0, 0, w, h);

        const data = this._data?.waveform_overview || this._data?.waveform_color;
        if (!data || data.length === 0) {
            this._renderMonoOverview(ctx, w, h);
        } else {
            // Rekordbox-style stacked bars: rendered from the bottom, no mirroring.
            // Bass (blue) fills from the bottom, mid (orange) stacks on top,
            // high (white) at the tip. Total bar height = overall amplitude.
            //
            // First pass: average each pixel's band amplitudes. Second: smooth
            // them with the envelope follower (a moving floor) so the preview
            // reads as a continuous silhouette instead of a spiky comb.
            const scale = h;
            const ampA = new Float32Array(w); // overall loudness → bar height
            const bandB = new Float32Array(w);
            const bandM = new Float32Array(w);
            const bandH = new Float32Array(w);

            for (let px = 0; px < w; px++) {
                const iStart = Math.floor(px * data.length / w);
                const iEnd = Math.min(data.length - 1, Math.floor((px + 1) * data.length / w));
                let sumAmp = 0, sumR = 0, sumG = 0, sumB = 0, count = 0;
                for (let i = iStart; i <= iEnd; i++) {
                    const d = data[i];
                    sumAmp += d.amp;
                    sumR += d.r; sumG += d.g; sumB += d.b; count++;
                }
                if (count === 0) continue;
                const avgAmp = sumAmp / count;
                ampA[px] = avgAmp;
                const [b, m, hi] = bandAmps(avgAmp, sumR / count, sumG / count, sumB / count);
                bandB[px] = b; bandM[px] = m; bandH[px] = hi;
            }

            // Envelope-smooth height + band split so the preview is a continuous
            // silhouette rather than a spiky comb.
            const ampE = applyEnvelope(ampA, 1, OVERVIEW_DECAY, OVERVIEW_ATTACK);
            const bE = applyEnvelope(bandB, 1, OVERVIEW_DECAY, OVERVIEW_ATTACK);
            const mE = applyEnvelope(bandM, 1, OVERVIEW_DECAY, OVERVIEW_ATTACK);
            const hE = applyEnvelope(bandH, 1, OVERVIEW_DECAY, OVERVIEW_ATTACK);

            for (let px = 0; px < w; px++) {
                if (ampE[px] < 0.005) continue;
                const total = bE[px] + mE[px] + hE[px];
                if (total <= 0) continue;
                const colH = ampE[px] * scale;
                const bassH = (bE[px] / total) * colH;
                const midH  = (mE[px] / total) * colH;
                const highH = (hE[px] / total) * colH;

                let y = h;
                ctx.fillStyle = COLOR_BASS;
                ctx.fillRect(px, y - bassH, 1, bassH);
                y -= bassH;
                ctx.fillStyle = COLOR_MID;
                ctx.fillRect(px, y - midH, 1, midH);
                y -= midH;
                ctx.fillStyle = COLOR_HIGH;
                ctx.fillRect(px, y - highH, 1, highH);
            }
        }

        // Cue ticks across the full track
        const cues = this._data?.cues;
        if (cues) {
            for (const cue of cues) {
                const frac = Math.max(0, Math.min(1, cue.position ?? 0));
                ctx.fillStyle = cue.color || '#4ade80';
                ctx.fillRect(frac * w - 0.5, 0, 1, h);
            }
        }
    }

    _renderMonoOverview(ctx, w, h) {
        const previewData = this._data?.waveform_preview;
        if (!previewData || previewData.length === 0) return;
        const ampA = new Float32Array(w);
        const brightA = new Float32Array(w);
        for (let px = 0; px < w; px++) {
            const di = Math.min(Math.floor(px * previewData.length / w), previewData.length - 1);
            const byte = previewData[di];
            ampA[px] = (byte & 0x1F) / 31.0;
            brightA[px] = 100 + ((byte >> 5) & 0x07) / 7.0 * 155;
        }
        const ampE = applyEnvelope(ampA, 1, OVERVIEW_DECAY, OVERVIEW_ATTACK);
        for (let px = 0; px < w; px++) {
            const barH = ampE[px] * h;
            if (barH < 0.5) continue;
            const brightness = Math.round(brightA[px]);
            ctx.fillStyle = `rgba(${brightness}, ${brightness}, ${brightness}, 0.7)`;
            ctx.fillRect(px, h - barH, 1, barH);
        }
    }

    // ── Zoom ──────────────────────────────────────────────────────────────

    _renderZoom() {
        const canvas = this._zoomCanvas;
        const rect = canvas.getBoundingClientRect();
        if (rect.width === 0) return;

        const w = rect.width;
        const h = rect.height;
        const ctx = this._resizeCanvas(canvas, w, h);

        ctx.fillStyle = '#0d0d0d';
        ctx.fillRect(0, 0, w, h);

        if (!this._data) return;

        const visibleFrac = 1.0 / this._zoom;
        const startFrac = this._offset;
        const endFrac = startFrac + visibleFrac;

        const colorData = this._data.waveform_color;
        if (colorData && colorData.length > 0) {
            this._renderLexiconWaveform(ctx, w, h, colorData, startFrac, endFrac);
        } else {
            this._renderMonoFallback(ctx, w, h, this._data.waveform_preview, startFrac, endFrac);
        }

        this._renderBeatGrid(ctx, w, h, startFrac, endFrac);
        this._renderTimeGrid(ctx, w, h, startFrac, endFrac);
        this._renderCues(ctx, w, h, startFrac, endFrac);
        this._renderCentrePlayhead(ctx, w, h);
    }

    // Fixed centre playhead for expanded mode — drawn last so it sits above the grid.
    _renderCentrePlayhead(ctx, w, h) {
        if (!this._lockPlayhead || this._playheadFrac < 0) return;
        const x = w / 2;
        ctx.strokeStyle = '#ffffff';
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

    _renderLexiconWaveform(ctx, w, h, data, startFrac, endFrac) {
        const N = data.length;
        if (N === 0) return;

        const visibleFrac = endFrac - startFrac;
        if (visibleFrac <= 0) return;

        const entriesPerPixel = (visibleFrac * N) / w;

        // ── Envelope lead-in ────────────────────────────────────────
        // Process extra pixels before the viewport so the envelope
        // follower is warmed up at the left edge.  Prevents shape
        // popping when transients cross the viewport boundary.
        const ENV_LEAD = 64;
        const leadFrac = ENV_LEAD * visibleFrac / w;
        // Allow negative — centred-at-start uses offset < 0 (blank before t=0).
        const extStartFrac = startFrac - leadFrac;
        const leadPx = Math.max(0, Math.round((startFrac - extStartFrac) / visibleFrac * w));
        const totalPx = leadPx + w;

        const bassA = new Float32Array(totalPx);
        const midA  = new Float32Array(totalPx);
        const highA = new Float32Array(totalPx);

        // ── Pixel-first sampling ────────────────────────────────────
        // Interpolate between columns when zoomed in (avoids entry-aligned
        // plateaus). Peak-hold per pixel when zoomed out.
        const pixelScale = w / visibleFrac;

        for (let px = 0; px < totalPx; px++) {
            const fracLo = extStartFrac + px / pixelScale;
            const fracHi = extStartFrac + (px + 1) / pixelScale;
            if (fracHi <= 0) continue;

            if (entriesPerPixel < 1) {
                const d = sampleColumnAtFrac(data, N, (fracLo + fracHi) * 0.5);
                const [b, m, hi] = bandAmps(d.amp, d.r, d.g, d.b);
                bassA[px] = b;
                midA[px]  = m;
                highA[px] = hi;
            } else {
                const iStart = Math.max(0, Math.floor(fracLo * N));
                const iEnd = Math.min(N - 1, Math.max(iStart, Math.ceil(fracHi * N) - 1));
                for (let i = iStart; i <= iEnd; i++) {
                    const d = data[i];
                    const [b, m, hi] = bandAmps(d.amp, d.r, d.g, d.b);
                    if (b > bassA[px]) bassA[px] = b;
                    if (m > midA[px]) midA[px] = m;
                    if (hi > highA[px]) highA[px] = hi;
                }
            }
        }

        // ── Envelope (full range including lead-in) ─────────────────
        // Pixel-space smoothing — same decay as the overview bar.
        const decay = this._mode === 'compact' ? 0.88 : 0.86;
        const bassE = applyEnvelope(bassA, 1, decay);
        const midE  = applyEnvelope(midA,  1, decay);
        const highE = applyEnvelope(highA, 1, decay);

        // ── Draw visible portion only (skip lead-in buffer) ─────────
        const yCenter = h / 2;
        const scale = h / 2;
        drawBand(ctx, bassE.subarray(leadPx, leadPx + w), w, yCenter, scale, COLOR_BASS);
        drawBand(ctx, midE.subarray(leadPx, leadPx + w),  w, yCenter, scale, COLOR_MID);
        drawBand(ctx, highE.subarray(leadPx, leadPx + w), w, yCenter, scale, COLOR_HIGH);
    }

    _renderMonoFallback(ctx, w, h, previewData, startFrac, endFrac) {
        if (!previewData || previewData.length === 0) return;
        const visibleFrac = endFrac - startFrac;
        if (visibleFrac <= 0) return;

        ctx.lineWidth = 2;
        ctx.lineCap = 'round';

        for (let px = 0; px < w; px++) {
            const fracLo = startFrac + (px / w) * visibleFrac;
            const fracHi = startFrac + ((px + 1) / w) * visibleFrac;
            if (fracHi <= 0) continue;
            const frac = Math.max(0, (fracLo + fracHi) * 0.5);
            const di = Math.min(Math.floor(frac * previewData.length), previewData.length - 1);
            const byte = previewData[di];
            const amplitude = (byte & 0x1F) / 31.0;
            const whiteness = ((byte >> 5) & 0x07) / 7.0;
            const brightness = Math.round(100 + whiteness * 155);

            const halfBar = amplitude * (h / 2);
            const yTop = h / 2 - halfBar;
            const yBottom = h / 2 + halfBar;

            ctx.strokeStyle = amplitude < 0.01
                ? FALLBACK_COLOR
                : `rgba(${brightness}, ${brightness}, ${brightness}, ${WAVEFORM_ALPHA})`;
            const x = px + 0.5;
            ctx.beginPath();
            ctx.moveTo(x, yBottom);
            ctx.lineTo(x, yTop);
            ctx.stroke();
        }
    }

    _renderBeatGrid(ctx, w, h, startFrac, endFrac) {
        const beats = this._data?.beats;
        const durationMs = this._data?.duration_ms;
        if (!beats || beats.length === 0 || !durationMs) return;

        // Compact shows the whole track, so a full per-beat grid is an
        // unreadable wash. Draw only phrase boundaries (every 4 bars) as thin
        // subtle ticks — enough rhythmic reference without the clutter.
        if (this._mode === 'compact') {
            this._renderCompactBeatGrid(ctx, w, h, startFrac, endFrac, beats, durationMs);
            return;
        }

        let barNumber = 0;

        for (const beat of beats) {
            const frac = beat.time_ms / durationMs;
            if (beat.bar_position === 1) barNumber++;
            if (frac < startFrac || frac > endFrac) continue;

            const x = ((frac - startFrac) / (endFrac - startFrac)) * w;
            const isBar = beat.bar_position === 1;
            const isPhrase = isBar && barNumber % 4 === 1;

            if (!isBar) {
                ctx.strokeStyle = 'rgba(255, 255, 255, 0.3)';
                ctx.lineWidth = 0.5;
                ctx.beginPath();
                ctx.moveTo(x, 0);
                ctx.lineTo(x, h);
                ctx.stroke();
            } else {
                ctx.strokeStyle = 'rgba(255, 255, 255, 0.7)';
                ctx.lineWidth = 1;
                ctx.beginPath();
                ctx.moveTo(x, 0);
                ctx.lineTo(x, h);
                ctx.stroke();

                // Bar number — flip away from the fixed centre playhead.
                const fontSize = 10;
                const label = String(barNumber);
                ctx.font = `${isPhrase ? 'bold ' : ''}${fontSize}px system-ui`;
                const labelW = ctx.measureText(label).width + 2;
                const nearCentre = Math.abs(x - w / 2) < 28;
                ctx.fillStyle = '#0d0d0d';
                if (nearCentre) {
                    ctx.textAlign = 'left';
                    ctx.fillRect(x + 2, 0, labelW + 2, fontSize + 4);
                    ctx.fillStyle = isPhrase ? 'rgba(255,255,255,0.9)' : 'rgba(255,255,255,0.6)';
                    ctx.fillText(label, x + 4, fontSize + 2);
                } else {
                    ctx.textAlign = 'right';
                    ctx.fillRect(x - labelW - 2, 0, labelW + 2, fontSize + 4);
                    ctx.fillStyle = isPhrase ? 'rgba(255,255,255,0.9)' : 'rgba(255,255,255,0.6)';
                    ctx.fillText(label, x - 2, fontSize + 2);
                }
            }
        }
    }

    // Phrase-only grid for the compact full-track view.
    _renderCompactBeatGrid(ctx, w, h, startFrac, endFrac, beats, durationMs) {
        const span = endFrac - startFrac;
        if (span <= 0) return;
        let barNumber = 0;
        ctx.strokeStyle = 'rgba(255, 255, 255, 0.28)';
        ctx.lineWidth = 1;
        for (const beat of beats) {
            if (beat.bar_position !== 1) continue;
            barNumber++;
            if (barNumber % 4 !== 1) continue; // phrase boundary only
            const frac = beat.time_ms / durationMs;
            if (frac < startFrac || frac > endFrac) continue;
            const x = ((frac - startFrac) / span) * w;
            ctx.beginPath();
            ctx.moveTo(x, 0);
            ctx.lineTo(x, h);
            ctx.stroke();
        }
    }

    _renderTimeGrid(ctx, w, h, startFrac, endFrac) {
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
        ctx.fillStyle = compact ? 'rgba(255, 255, 255, 0.22)' : 'rgba(255, 255, 255, 0.35)';
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

    // Track fraction (0–1) under a clientX on the zoom canvas, honouring the
    // current viewport. In compact (zoom 1, offset 0) this is just x/width.
    _zoomFracAtClientX(clientX) {
        const rect = this._zoomCanvas.getBoundingClientRect();
        const clickFrac = (clientX - rect.left) / rect.width;
        const visibleFrac = 1.0 / this._zoom;
        return Math.max(0, Math.min(1, this._offset + clickFrac * visibleFrac));
    }

    _bindEvents() {
        // Overview click (expanded only) seeks to the clicked fraction.
        this._onOverviewClick = (e) => {
            const rect = this._overviewCanvas.getBoundingClientRect();
            const frac = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
            this._playheadFrac = frac;
            this._phClock = frac;
            this._phTarget = frac;
            this._phTargetAt = this._now();
            if (this._lockPlayhead) this._offset = this._centeredOffset();
            this._render();
            this._updatePlayheadOverlay();
            if (this.onSeek) this.onSeek(frac);
        };

        this._onMouseDown = (e) => {
            if (e.button !== 0) return;
            this._dragging = true;
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
                this._zoomCanvas.style.cursor = 'ew-resize';
            }

            // Visual-only: move the playhead now, defer the seek to mouseup so we
            // don't flood the audio backend. In expanded the waveform scrolls
            // under the centred playhead.
            let frac;
            if (this._lockPlayhead) {
                const rect = this._zoomCanvas.getBoundingClientRect();
                const visibleFrac = 1.0 / this._zoom;
                frac = this._dragStartPlayhead - (dx / rect.width) * visibleFrac;
            } else {
                frac = this._zoomFracAtClientX(e.clientX);
            }
            this._playheadFrac = Math.max(0, Math.min(1, frac));
            if (this._lockPlayhead) this._offset = this._centeredOffset();
            this._render();
            this._updatePlayheadOverlay();
        };

        this._onMouseUp = (e) => {
            if (!this._dragging) return;
            this._dragging = false;
            this._zoomCanvas.style.cursor = this._defaultCursor();

            // Scrub drag or a plain click — issue the actual seek here.
            const frac = this._gesture === 'scrub'
                ? this._playheadFrac
                : this._zoomFracAtClientX(e.clientX);
            this._gesture = 'none';
            this._scrubbing = false;
            if (this.onSeek && this._data?.duration_ms) {
                this._playheadFrac = Math.max(0, Math.min(1, frac));
                this._phClock = this._playheadFrac;
                this._phTarget = this._playheadFrac;
                this._phTargetAt = this._now();
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
        this._overviewCanvas.addEventListener('click', this._onOverviewClick);
        this._zoomCanvas.addEventListener('mousedown', this._onMouseDown);
        window.addEventListener('mousemove', this._onMouseMove);
        window.addEventListener('mouseup', this._onMouseUp);
        this._zoomCanvas.addEventListener('wheel', this._onWheel, { passive: false });
    }

    _unbindEvents() {
        this._overviewCanvas.removeEventListener('click', this._onOverviewClick);
        this._zoomCanvas.removeEventListener('mousedown', this._onMouseDown);
        window.removeEventListener('mousemove', this._onMouseMove);
        window.removeEventListener('mouseup', this._onMouseUp);
        this._zoomCanvas.removeEventListener('wheel', this._onWheel);
    }
}
