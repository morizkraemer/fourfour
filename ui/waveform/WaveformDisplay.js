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
 *     duration_ms:      number,
 *     bpm:              number,
 *   }
 */

const FALLBACK_COLOR = 'rgb(80, 80, 80)';

// Rekordbox 3-band colors: bass=blue, mids=orange, highs=white
const COLOR_BASS = 'rgb(30,  100, 255)';
const COLOR_MID  = 'rgb(255, 140,   0)';
const COLOR_HIGH = 'rgb(255, 255, 255)';

// Given per-pixel overall amplitude + band weights (0-255, already relative
// to the dominant band), return absolute amplitudes for each band.
function bandAmps(amp, bassW, midW, highW) {
    const s = amp / 255;
    return [bassW * s, midW * s, highW * s];
}

// Apply an envelope follower: instant attack, zoom-invariant exponential decay.
// decayPerColumn controls how fast the tail falls after a transient — the same
// number of waveform columns regardless of current zoom level.
function applyEnvelope(amps, samplesPerPixel, decayPerColumn = 0.82) {
    const out = new Float32Array(amps.length);
    // Convert column-space decay to pixel-space so it's zoom-invariant.
    // Zoomed out (spp > 1): many columns per pixel → fast pixel decay.
    // Zoomed in  (spp < 1): fraction of a column per pixel → slow pixel decay.
    const decayPerPx = Math.pow(decayPerColumn, Math.max(samplesPerPixel, 0.01));
    let env = 0;
    for (let i = 0; i < amps.length; i++) {
        if (amps[i] > env) {
            env = amps[i]; // instant attack
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
    /** @param {HTMLElement} container */
    constructor(container) {
        this._container = container;

        // Create wrapper div
        this._wrapper = document.createElement('div');
        this._wrapper.className = 'waveform-display';

        // Overview canvas (full-track mini-view) — wrapped for playhead overlay
        this._overviewCanvas = document.createElement('canvas');
        this._overviewCanvas.className = 'waveform-display__overview';
        const overviewWrap = document.createElement('div');
        overviewWrap.style.cssText = 'position:relative;flex-shrink:0;';
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
            'position:absolute;top:0;bottom:0;width:1px;background:#fff;pointer-events:none;display:none;transform:translateX(-50%)';
        zoomWrap.appendChild(this._zoomPlayhead);

        this._wrapper.appendChild(overviewWrap);
        this._wrapper.appendChild(zoomWrap);
        container.appendChild(this._wrapper);

        // State
        this._data = null;
        this._zoom = 1.0;
        this._offset = 0.0;
        this._dragging = false;
        this._dragStartX = 0;
        this._dragStartY = 0;
        this._dragStartOffset = 0;
        this._dragStartZoom = 1.0;
        this._dragMoved = false;
        this._playheadFrac = -1;

        // rAF handle — coalesces rapid scroll/drag events into one draw per frame
        this._zoomRafId = null;

        this._bindEvents();
    }

    /** Replace data and re-render. Resets zoom and scroll position. */
    setData(data) {
        this._data = data;
        this._zoom = 1.0;
        this._offset = 0.0;
        this._render();
    }

/** Clear to empty state. */
    clear() {
        this._data = null;
        this._zoom = 1.0;
        this._offset = 0.0;
        this._render();
    }

    /** Re-render at current size — call after container resize. */
    redraw() {
        this._render();
    }

    /** Set playhead position (0–1 fraction of duration). Pass -1 to hide. */
    setPlayhead(frac) {
        this._playheadFrac = frac;
        this._updatePlayheadOverlay();
    }

    _updatePlayheadOverlay() {
        const frac = this._playheadFrac;
        if (frac < 0 || frac > 1) {
            this._overviewPlayhead.style.display = 'none';
            this._zoomPlayhead.style.display = 'none';
            return;
        }
        this._overviewPlayhead.style.display = 'block';
        this._overviewPlayhead.style.left = (frac * 100) + '%';
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

    /** Remove canvases and all event listeners. */
    destroy() {
        if (this._zoomRafId) cancelAnimationFrame(this._zoomRafId);
        this._unbindEvents();
        this._wrapper.remove();
    }

    // ── Private ──────────────────────────────────────────────────────────

    _render() {
        this._renderOverview();
        // Coalesce rapid scroll/drag events — only draw once per animation frame
        if (this._zoomRafId) cancelAnimationFrame(this._zoomRafId);
        this._zoomRafId = requestAnimationFrame(() => {
            this._zoomRafId = null;
            this._renderZoom();
            this._updatePlayheadOverlay();
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
        const canvas = this._overviewCanvas;
        const rect = canvas.getBoundingClientRect();
        if (rect.width === 0) return;

        const w = rect.width;
        const h = rect.height;
        const ctx = this._resizeCanvas(canvas, w, h);

        ctx.fillStyle = '#0d0d0d';
        ctx.fillRect(0, 0, w, h);

        const data = this._data?.waveform_color;
        if (!data || data.length === 0) {
            this._renderMonoOverview(ctx, w, h);
        } else {
            // Rekordbox-style stacked bars: rendered from the bottom, no mirroring.
            // Bass (blue) fills from the bottom, mid (orange) stacks on top,
            // high (white) at the tip. Total bar height = overall amplitude.
            // Color fractions = relative band contributions, normalised to sum to 1.
            const scale = h * 0.95;

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
                const maxAmp = sumAmp / count;
                if (maxAmp < 0.005) continue;

                const [bA, mA, hA] = bandAmps(maxAmp, sumR / count, sumG / count, sumB / count);
                const total = bA + mA + hA;
                if (total <= 0) continue;

                // Total column height proportional to amplitude
                const colH = maxAmp * scale;

                // Each band's share of the column, stacked bottom → top
                const bassH = (bA / total) * colH;
                const midH  = (mA / total) * colH;
                const highH = (hA / total) * colH;

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

        // Viewport indicator
        const visibleFrac = 1.0 / this._zoom;
        const x1 = this._offset * w;
        const x2 = (this._offset + visibleFrac) * w;
        ctx.fillStyle = 'rgba(255, 255, 255, 0.08)';
        ctx.fillRect(x1, 0, x2 - x1, h);
        ctx.strokeStyle = 'rgba(255, 255, 255, 0.3)';
        ctx.lineWidth = 1;
        ctx.strokeRect(x1, 0, x2 - x1, h);

    }

    _renderMonoOverview(ctx, w, h) {
        const previewData = this._data?.waveform_preview;
        if (!previewData || previewData.length === 0) return;
        // Monochrome fallback: stacked bars from bottom, no mirroring.
        for (let px = 0; px < w; px++) {
            const di = Math.min(Math.floor(px * previewData.length / w), previewData.length - 1);
            const byte = previewData[di];
            const amplitude = (byte & 0x1F) / 31.0;
            const whiteness = ((byte >> 5) & 0x07) / 7.0;
            const brightness = Math.round(100 + whiteness * 155);
            const barH = amplitude * h * 0.95;
            if (barH < 0.5) continue;
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
        const extStartFrac = Math.max(0, startFrac - leadFrac);
        const leadPx = Math.round((startFrac - extStartFrac) / visibleFrac * w);
        const totalPx = leadPx + w;

        const bassA = new Float32Array(totalPx);
        const midA  = new Float32Array(totalPx);
        const highA = new Float32Array(totalPx);

        // ── Data → pixel  (iterate entries, not pixels) ─────────────
        // Each data entry maps to a deterministic pixel position.
        // On pan ALL entries shift by the same pixel offset, so the
        // waveform shape slides without morphing.
        const pixelScale = w / visibleFrac;          // px per track-fraction
        const iFirst = Math.max(0, Math.floor(extStartFrac * N));
        const iLast  = Math.min(N - 1, Math.ceil(endFrac * N));

        if (entriesPerPixel < 1) {
            // Zoomed in — each entry spans multiple pixels.
            // Fill each entry's pixel range with its amplitude.
            for (let i = iFirst; i <= iLast; i++) {
                const px0 = Math.max(0, Math.floor((i / N - extStartFrac) * pixelScale));
                const px1 = Math.min(totalPx, Math.ceil(((i + 1) / N - extStartFrac) * pixelScale));
                const d = data[i];
                const [b, m, hi] = bandAmps(d.amp, d.r, d.g, d.b);
                for (let px = px0; px < px1; px++) {
                    bassA[px] = b;
                    midA[px]  = m;
                    highA[px] = hi;
                }
            }
        } else {
            // Zoomed out — multiple entries per pixel.
            // MAX preserves transient peaks and is far more stable than
            // averaging when entries cross pixel boundaries on pan.
            for (let i = iFirst; i <= iLast; i++) {
                const px = Math.floor((i / N - extStartFrac) * pixelScale);
                if (px < 0 || px >= totalPx) continue;
                const d = data[i];
                const [b, m, hi] = bandAmps(d.amp, d.r, d.g, d.b);
                if (b > bassA[px]) bassA[px] = b;
                if (m > midA[px]) midA[px] = m;
                if (hi > highA[px]) highA[px] = hi;
            }
        }

        // ── Envelope (full range including lead-in) ─────────────────
        const bassE = applyEnvelope(bassA, entriesPerPixel, 0.92);
        const midE  = applyEnvelope(midA,  entriesPerPixel, 0.92);
        const highE = applyEnvelope(highA, entriesPerPixel, 0.92);

        // ── Draw visible portion only (skip lead-in buffer) ─────────
        const yCenter = h / 2;
        const scale = h / 2 * 0.95;
        drawBand(ctx, bassE.subarray(leadPx, leadPx + w), w, yCenter, scale, COLOR_BASS);
        drawBand(ctx, midE.subarray(leadPx, leadPx + w),  w, yCenter, scale, COLOR_MID);
        drawBand(ctx, highE.subarray(leadPx, leadPx + w), w, yCenter, scale, COLOR_HIGH);
    }

    _renderMonoFallback(ctx, w, h, previewData, startFrac, endFrac) {
        if (!previewData || previewData.length === 0) return;
        const startIdx = Math.floor(startFrac * previewData.length);
        const endIdx = Math.ceil(endFrac * previewData.length);
        const visibleCount = endIdx - startIdx;
        if (visibleCount <= 0) return;
        const colW = w / visibleCount;

        ctx.lineWidth = 2;
        ctx.lineCap = 'round';

        for (let i = 0; i < visibleCount; i++) {
            const di = startIdx + i;
            if (di >= previewData.length) break;
            const byte = previewData[di];
            const amplitude = (byte & 0x1F) / 31.0;
            const whiteness = ((byte >> 5) & 0x07) / 7.0;
            const brightness = Math.round(100 + whiteness * 155);

            const halfBar = amplitude * (h / 2) * 0.95;
            const yTop = h / 2 - halfBar;
            const yBottom = h / 2 + halfBar;

            ctx.strokeStyle = amplitude < 0.01
                ? FALLBACK_COLOR
                : `rgba(${brightness}, ${brightness}, ${brightness}, ${WAVEFORM_ALPHA})`;
            ctx.beginPath();
            ctx.moveTo(i * colW + colW / 2, yBottom);
            ctx.lineTo(i * colW + colW / 2, yTop);
            ctx.stroke();
        }
    }

    _renderBeatGrid(ctx, w, h, startFrac, endFrac) {
        const beats = this._data?.beats;
        const durationMs = this._data?.duration_ms;
        const bpm = this._data?.bpm;
        if (!beats || beats.length === 0 || !durationMs) return;

        let barNumber = 0;
        let firstBeatDrawn = false;

        for (const beat of beats) {
            const frac = beat.time_ms / durationMs;
            if (beat.bar_position === 1) barNumber++;
            if (frac < startFrac || frac > endFrac) continue;

            const x = ((frac - startFrac) / (endFrac - startFrac)) * w;
            const isBar = beat.bar_position === 1;
            const isPhrase = isBar && barNumber % 4 === 1;
            const isFirst = !firstBeatDrawn && isBar;
            if (isBar) firstBeatDrawn = true;

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

                // Bar number
                const fontSize = 10;
                const label = String(barNumber);
                ctx.font = `${isPhrase ? 'bold ' : ''}${fontSize}px system-ui`;
                ctx.textAlign = 'right';
                const labelW = ctx.measureText(label).width + 2;
                ctx.fillStyle = '#0d0d0d';
                ctx.fillRect(x - labelW - 2, 0, labelW + 2, fontSize + 4);
                ctx.fillStyle = isPhrase ? 'rgba(255,255,255,0.9)' : 'rgba(255,255,255,0.6)';
                ctx.fillText(label, x - 2, fontSize + 2);

                // First beat BPM pill
                if (isFirst && bpm) {
                    const bpmLabel = `${bpm.toFixed(2)} BPM`;
                    ctx.font = '10px system-ui';
                    const pillW = ctx.measureText(bpmLabel).width + 8;
                    const pillH = 16;
                    const pillY = h - pillH - 2;
                    ctx.fillStyle = '#5ae168';
                    ctx.beginPath();
                    ctx.roundRect(x, pillY, pillW, pillH, [0, 3, 3, 0]);
                    ctx.fill();
                    ctx.fillStyle = '#000';
                    ctx.textAlign = 'left';
                    ctx.fillText(bpmLabel, x + 4, pillY + 11);
                }
            }
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

        ctx.fillStyle = 'rgba(255, 255, 255, 0.35)';
        ctx.font = '9px system-ui';
        ctx.textAlign = 'left';

        for (let ms = firstTick; ms <= endMs; ms += intervalMs) {
            const x = ((ms / durationMs - startFrac) / (endFrac - startFrac)) * w;
            const secs = Math.floor(ms / 1000);
            ctx.fillText(
                `${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, '0')}`,
                x + 2, h - 3,
            );
        }
    }

    // ── Interaction ───────────────────────────────────────────────────────

    _clampOffset(offset) {
        return Math.max(0, Math.min(1.0 - 1.0 / this._zoom, offset));
    }

    _bindEvents() {
        this._onOverviewClick = (e) => {
            const rect = this._overviewCanvas.getBoundingClientRect();
            const frac = (e.clientX - rect.left) / rect.width;
            this._offset = this._clampOffset(frac - 0.5 / this._zoom);
            this._render();
            if (this.onViewportChange) this.onViewportChange(this._zoom, this._offset);
            if (this.onSeek) this.onSeek(frac);
        };

        this._onMouseDown = (e) => {
            this._dragging = true;
            this._dragMoved = false;
            this._dragStartX = e.clientX;
            this._dragStartY = e.clientY;
            this._dragStartOffset = this._offset;
            this._dragStartZoom = this._zoom;
            this._zoomCanvas.style.cursor = 'grabbing';
            e.preventDefault();
        };

        this._onMouseMove = (e) => {
            if (!this._dragging) return;
            const dy = this._dragStartY - e.clientY;
            if (Math.abs(e.clientX - this._dragStartX) > 4 || Math.abs(dy) > 4) this._dragMoved = true;
            const dx = e.clientX - this._dragStartX;
            this._zoom = Math.max(1.0, Math.min(512.0, this._dragStartZoom * Math.pow(1.015, dy)));
            if (this._zoom > 1.0) {
                const rect = this._zoomCanvas.getBoundingClientRect();
                this._offset = this._clampOffset(this._dragStartOffset - dx / rect.width / this._zoom);
            }
            this._render();
            if (this.onViewportChange) this.onViewportChange(this._zoom, this._offset);
        };

        this._onMouseUp = (e) => {
            if (this._dragging) {
                if (!this._dragMoved && this.onSeek && this._data?.duration_ms) {
                    const rect = this._zoomCanvas.getBoundingClientRect();
                    const clickFrac = (e.clientX - rect.left) / rect.width;
                    const visibleFrac = 1.0 / this._zoom;
                    const seekFrac = this._offset + clickFrac * visibleFrac;
                    this.onSeek(Math.max(0, Math.min(1, seekFrac)));
                }
                this._dragging = false;
                this._zoomCanvas.style.cursor = 'default';
            }
        };

        this._onWheel = (e) => {
            if (this._zoom <= 1.0) return;
            e.preventDefault();
            this._offset = this._clampOffset(this._offset + (e.deltaX + e.deltaY) * 0.005 / this._zoom);
            this._render();
            if (this.onViewportChange) this.onViewportChange(this._zoom, this._offset);
        };

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
