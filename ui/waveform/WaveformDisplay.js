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
const WAVEFORM_ALPHA = 0.7;

export default class WaveformDisplay {
    /** @param {HTMLElement} container */
    constructor(container) {
        this._container = container;

        // Create wrapper div
        this._wrapper = document.createElement('div');
        this._wrapper.className = 'waveform-display';

        // Overview canvas (full-track mini-view)
        this._overviewCanvas = document.createElement('canvas');
        this._overviewCanvas.className = 'waveform-display__overview';

        // Zoom canvas (scrollable detail view)
        this._zoomCanvas = document.createElement('canvas');
        this._zoomCanvas.className = 'waveform-display__zoom';

        this._wrapper.appendChild(this._overviewCanvas);
        this._wrapper.appendChild(this._zoomCanvas);
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

        this._bindEvents();
    }

    /** Replace data and re-render. */
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

    /** Remove canvases and all event listeners. */
    destroy() {
        this._unbindEvents();
        this._wrapper.remove();
    }

    // ── Private ──────────────────────────────────────────────────────────

    _render() {
        this._renderOverview();
        this._renderZoom();
    }

    // ── Overview ─────────────────────────────────────────────────────────

    _renderOverview() {
        const canvas = this._overviewCanvas;
        const rect = canvas.getBoundingClientRect();
        if (rect.width === 0) return;

        canvas.width = rect.width * devicePixelRatio;
        canvas.height = rect.height * devicePixelRatio;
        const ctx = canvas.getContext('2d');
        ctx.scale(devicePixelRatio, devicePixelRatio);
        const w = rect.width;
        const h = rect.height;

        ctx.fillStyle = '#0d0d0d';
        ctx.fillRect(0, 0, w, h);

        const data = this._data?.waveform_color;
        if (!data || data.length === 0) {
            this._renderMonoOverview(ctx, w, h);
            return;
        }

        // One column per CSS pixel — draw upward from center (Lexicon overview style)
        let prevR = 0, prevG = 0, prevB = 0;
        for (let px = 0; px < w; px++) {
            const idx = Math.min(Math.floor(px * data.length / w), data.length - 1);
            const { amp, r, g, b } = data[idx];

            const sr = Math.round(prevR * 0.5 + r * 0.5);
            const sg = Math.round(prevG * 0.5 + g * 0.5);
            const sb = Math.round(prevB * 0.5 + b * 0.5);
            prevR = r; prevG = g; prevB = b;

            // Lexicon overview: moveTo center, lineTo center - rmsHeight (upward only)
            const barH = amp * (h / 2) * 2 * 0.9;
            const yCenter = h / 2;

            ctx.strokeStyle = amp < 0.005
                ? FALLBACK_COLOR
                : `rgba(${sr}, ${sg}, ${sb}, ${WAVEFORM_ALPHA})`;
            ctx.lineWidth = 1;
            ctx.beginPath();
            ctx.moveTo(px, yCenter);
            ctx.lineTo(px, yCenter - barH);
            ctx.stroke();
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
        const yCenter = h / 2;
        for (let px = 0; px < w; px++) {
            const di = Math.min(Math.floor(px * previewData.length / w), previewData.length - 1);
            const byte = previewData[di];
            const amplitude = (byte & 0x1F) / 31.0;
            const whiteness = ((byte >> 5) & 0x07) / 7.0;
            const brightness = Math.round(100 + whiteness * 155);
            const barH = amplitude * (h / 2) * 2 * 0.9;
            ctx.strokeStyle = amplitude < 0.01
                ? 'rgb(80, 80, 80)'
                : `rgba(${brightness}, ${brightness}, ${brightness}, 0.7)`;
            ctx.lineWidth = 1;
            ctx.beginPath();
            ctx.moveTo(px, yCenter);
            ctx.lineTo(px, yCenter - barH);
            ctx.stroke();
        }
    }

    // ── Zoom ──────────────────────────────────────────────────────────────

    _renderZoom() {
        const canvas = this._zoomCanvas;
        const rect = canvas.getBoundingClientRect();
        if (rect.width === 0) return;

        canvas.width = rect.width * devicePixelRatio;
        canvas.height = rect.height * devicePixelRatio;
        const ctx = canvas.getContext('2d');
        ctx.scale(devicePixelRatio, devicePixelRatio);
        const w = rect.width;
        const h = rect.height;

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
        const startIdx = Math.floor(startFrac * data.length);
        const endIdx = Math.ceil(endFrac * data.length);
        const visibleCount = endIdx - startIdx;
        if (visibleCount <= 0) return;

        const samplesPerPixel = visibleCount / w;
        ctx.lineWidth = 2;
        ctx.lineCap = 'round';

        let prevR = 0, prevG = 0, prevB = 0;

        for (let px = 0; px < w; px++) {
            const tStart = startIdx + px * samplesPerPixel;
            const iStart = Math.max(0, Math.floor(tStart));
            const iEnd = Math.min(data.length - 1, Math.ceil(tStart + samplesPerPixel));
            if (iStart > iEnd) continue;

            let maxAmp = 0, sumR = 0, sumG = 0, sumB = 0, count = 0;

            if (samplesPerPixel <= 10) {
                // Zoomed in: max amplitude in range
                for (let i = iStart; i <= iEnd; i++) {
                    const d = data[i];
                    if (d.amp > maxAmp) maxAmp = d.amp;
                    sumR += d.r; sumG += d.g; sumB += d.b; count++;
                }
            } else {
                // Zoomed out: top-5 peaks weighted (Lexicon algorithm)
                const slice = [];
                for (let i = iStart; i <= iEnd; i++) {
                    slice.push(data[i]);
                    sumR += data[i].r; sumG += data[i].g; sumB += data[i].b; count++;
                }
                slice.sort((a, b) => b.amp - a.amp);
                const top = slice.slice(0, 5);
                let weightedAmp = 0, totalWeight = 0;
                top.forEach((entry, i) => {
                    const weight = top.length - i;
                    weightedAmp += entry.amp * weight;
                    totalWeight += weight;
                });
                maxAmp = totalWeight > 0 ? weightedAmp / totalWeight : 0;
            }

            const r = count > 0 ? Math.round(sumR / count) : 0;
            const g = count > 0 ? Math.round(sumG / count) : 0;
            const b = count > 0 ? Math.round(sumB / count) : 0;

            // 50% smooth with previous pixel
            const sr = Math.round(prevR * 0.5 + r * 0.5);
            const sg = Math.round(prevG * 0.5 + g * 0.5);
            const sb = Math.round(prevB * 0.5 + b * 0.5);
            prevR = r; prevG = g; prevB = b;

            // y = height - value * height + height/4 (Lexicon bottom-anchor formula)
            const yTop = h - maxAmp * h + h / 4;
            const yBottom = h + h / 4;  // off-canvas, clipped at canvas edge

            ctx.strokeStyle = Math.abs(yBottom - yTop) < 0.5
                ? FALLBACK_COLOR
                : `rgba(${sr}, ${sg}, ${sb}, ${WAVEFORM_ALPHA})`;
            ctx.beginPath();
            ctx.moveTo(px, yBottom);
            ctx.lineTo(px, yTop);
            ctx.stroke();
        }
    }

    _renderMonoFallback(ctx, w, h, previewData, startFrac, endFrac) {
        if (!previewData || previewData.length === 0) return;
        const startIdx = Math.floor(startFrac * previewData.length);
        const endIdx = Math.ceil(endFrac * previewData.length);
        const visibleCount = endIdx - startIdx;
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

            const yTop = h - amplitude * h + h / 4;
            const yBottom = h + h / 4;

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
        };

        this._onMouseDown = (e) => {
            this._dragging = true;
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
            const dx = e.clientX - this._dragStartX;
            this._zoom = Math.max(1.0, Math.min(64.0, this._dragStartZoom * Math.pow(1.015, dy)));
            if (this._zoom > 1.0) {
                const rect = this._zoomCanvas.getBoundingClientRect();
                this._offset = this._clampOffset(this._dragStartOffset - dx / rect.width / this._zoom);
            }
            this._render();
        };

        this._onMouseUp = () => {
            if (this._dragging) {
                this._dragging = false;
                this._zoomCanvas.style.cursor = 'default';
            }
        };

        this._onWheel = (e) => {
            if (this._zoom <= 1.0) return;
            e.preventDefault();
            this._offset = this._clampOffset(this._offset + (e.deltaX + e.deltaY) * 0.005 / this._zoom);
            this._render();
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
