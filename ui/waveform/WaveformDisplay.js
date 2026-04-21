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

// Gaussian-weighted smooth over a pixel window — blends bar heights so the
// waveform flows continuously rather than stepping between columns.
function smoothAmps(arr, radius = 2) {
    const out = new Float32Array(arr.length);
    // Weights: centre=4, ±1=2, ±2=1  (approximates a Gaussian)
    const kernel = [1, 2, 4, 2, 1];
    const kSum = 10;
    for (let i = 0; i < arr.length; i++) {
        let v = 0;
        for (let k = -radius; k <= radius; k++) {
            const j = Math.min(arr.length - 1, Math.max(0, i + k));
            v += arr[j] * kernel[k + radius];
        }
        out[i] = v / kSum;
    }
    return out;
}


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

        // Pre-compute per-pixel absolute band amplitudes (max scan per bucket)
        const bassA = new Float32Array(w);
        const midA  = new Float32Array(w);
        const highA = new Float32Array(w);

        for (let px = 0; px < w; px++) {
            const iStart = Math.floor(px * data.length / w);
            const iEnd = Math.min(data.length - 1, Math.floor((px + 1) * data.length / w));
            let maxAmp = 0, sumR = 0, sumG = 0, sumB = 0, count = 0;
            for (let i = iStart; i <= iEnd; i++) {
                const d = data[i];
                if (d.amp > maxAmp) maxAmp = d.amp;
                sumR += d.r; sumG += d.g; sumB += d.b; count++;
            }
            if (count === 0) continue;
            [bassA[px], midA[px], highA[px]] = bandAmps(maxAmp, sumR / count, sumG / count, sumB / count);
        }

        // Draw three layers back-to-front: bass (tallest/widest) → mids → highs (shortest)
        const yCenter = h / 2;
        const scale = h / 2 * 0.9;
        function overviewLayer(amps, color) {
            ctx.strokeStyle = color;
            ctx.lineWidth = 1;
            ctx.beginPath();
            for (let px = 0; px < w; px++) {
                if (amps[px] < 0.005) continue;
                const barH = amps[px] * scale;
                ctx.moveTo(px, yCenter - barH);
                ctx.lineTo(px, yCenter + barH);
            }
            ctx.stroke();
        }
        overviewLayer(smoothAmps(bassA), COLOR_BASS);
        overviewLayer(smoothAmps(midA),  COLOR_MID);
        overviewLayer(smoothAmps(highA), COLOR_HIGH);

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
            const barH = amplitude * (h / 2) * 0.9;
            ctx.strokeStyle = amplitude < 0.01
                ? 'rgb(80, 80, 80)'
                : `rgba(${brightness}, ${brightness}, ${brightness}, 0.7)`;
            ctx.lineWidth = 1;
            ctx.beginPath();
            ctx.moveTo(px, yCenter - barH);
            ctx.lineTo(px, yCenter + barH);
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

        // Pre-compute per-pixel absolute band amplitudes
        const bassA = new Float32Array(w);
        const midA  = new Float32Array(w);
        const highA = new Float32Array(w);

        for (let px = 0; px < w; px++) {
            const tStart = startIdx + px * samplesPerPixel;
            let amp, bassW, midW, highW;

            if (samplesPerPixel < 1) {
                // Zoomed in — interpolate between adjacent columns
                const i0 = Math.max(0, Math.floor(tStart));
                const i1 = Math.min(data.length - 1, i0 + 1);
                const t = tStart - i0;
                const d0 = data[i0], d1 = data[i1];
                amp   = d0.amp * (1 - t) + d1.amp * t;
                bassW = d0.r   * (1 - t) + d1.r   * t;
                midW  = d0.g   * (1 - t) + d1.g   * t;
                highW = d0.b   * (1 - t) + d1.b   * t;
            } else {
                // Zoomed out — max amplitude in range, average band weights
                const iStart = Math.max(0, Math.floor(tStart));
                const iEnd = Math.min(data.length - 1, Math.ceil(tStart + samplesPerPixel));
                if (iStart > iEnd) continue;
                let sumR = 0, sumG = 0, sumB = 0, count = 0;
                amp = 0;
                for (let i = iStart; i <= iEnd; i++) {
                    const d = data[i];
                    if (d.amp > amp) amp = d.amp;
                    sumR += d.r; sumG += d.g; sumB += d.b; count++;
                }
                if (count === 0) continue;
                bassW = sumR / count;
                midW  = sumG / count;
                highW = sumB / count;
            }
            [bassA[px], midA[px], highA[px]] = bandAmps(amp, bassW, midW, highW);
        }

        // Draw three layers back-to-front: bass → mids → highs
        const yCenter = h / 2;
        const scale = h / 2 * 0.95;
        ctx.lineWidth = 1.5;
        ctx.lineCap = 'round';

        function zoomLayer(amps, color) {
            ctx.strokeStyle = color;
            ctx.beginPath();
            for (let px = 0; px < w; px++) {
                if (amps[px] < 0.01) continue;
                const barH = amps[px] * scale;
                ctx.moveTo(px, yCenter - barH);
                ctx.lineTo(px, yCenter + barH);
            }
            ctx.stroke();
        }
        zoomLayer(smoothAmps(bassA), COLOR_BASS);
        zoomLayer(smoothAmps(midA),  COLOR_MID);
        zoomLayer(smoothAmps(highA), COLOR_HIGH);
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
