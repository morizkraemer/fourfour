import '../index.js';
import { el } from '../../../../viewport/design-system/utils/dom.js';
import { ZOOM_PEAK_WEIGHT, reduceBands } from '../../../../viewport/design-system/waveform-bands.js';
import { wavePalette } from '../../../../viewport/design-system/waveform-render.js';
import { paintSampleDots, paintZoom } from '../../../../viewport/design-system/waveform-zoom.js';
import fixture from './zoom-look-dendy.json';

export const LEVELS_MS = [5000, 1000, 300, 50, 10];
const WIDTH_CSS = 1100;
const HEIGHT_CSS = 132;
const MIN_SPAN_MS = 4;
const MAX_SPAN_MS = 5000;

function b64(text, Type) {
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return new Type(bytes.buffer);
}

let sources = null;

function loadSources() {
  if (sources) return sources;
  const d = fixture.detail;
  const unit = 1 / 127;
  const detailLow = b64(d.low, Uint8Array);
  const detail = {
    low: detailLow, mid: b64(d.mid, Uint8Array), high: b64(d.high, Uint8Array),
    scale: { low: unit, mid: unit, high: unit },
    signalLow: b64(d.signalLow, Int8Array), signalHigh: b64(d.signalHigh, Int8Array), signalScale: unit,
    startMs: d.startMs,
    msPerColumn: 1000 / d.columnsPerSecond,
  };
  const w = fixture.window;
  const lowest = b64(w.lowest, Int16Array);
  const win = {
    low: b64(w.low, Uint8Array), mid: b64(w.mid, Uint8Array), high: b64(w.high, Uint8Array),
    scale: { low: unit, mid: unit, high: unit },
    signalLow: lowest, signalHigh: b64(w.highest, Int16Array), signalScale: d.signalGain / (32767 * 127),
    startMs: ((w.firstSample - 0.5) / w.sampleRate) * 1000,
    msPerColumn: 1000 / w.sampleRate,
    firstSample: w.firstSample,
  };
  sources = { detail, win, centreMs: fixture.centreMs };
  return sources;
}

/** Paint the track around the fixture's centre with `spanMs` across the canvas, as the zoom view would. */
function paintView(canvas, look, spanMs) {
  const dpr = window.devicePixelRatio || 1;
  const widthPx = Math.round(WIDTH_CSS * dpr);
  const heightPx = Math.round(HEIGHT_CSS * dpr);
  if (canvas.width !== widthPx) canvas.width = widthPx;
  if (canvas.height !== heightPx) canvas.height = heightPx;
  const ctx = canvas.getContext('2d');
  const palette = wavePalette();
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.fillStyle = palette.bg;
  ctx.fillRect(0, 0, widthPx, heightPx);
  const { detail, win, centreMs } = loadSources();
  const startMs = centreMs - spanMs / 2;
  const msPerPx = spanMs / widthPx;
  const source = msPerPx >= detail.msPerColumn ? detail : win;
  const n = source.low.length;
  const durationMs = n * source.msPerColumn;
  const a = (startMs - source.startMs) / durationMs;
  const b = (startMs + spanMs - source.startMs) / durationMs;
  const cols = reduceBands(source, a, b, widthPx, ZOOM_PEAK_WEIGHT);
  paintZoom(ctx, cols, { look, heightPx, palette, msPerPx, dpr });
  if (source === win) {
    const first = Math.max(0, Math.floor((startMs - win.startMs) / win.msPerColumn) - 1);
    const last = Math.min(n - 1, Math.ceil((startMs + spanMs - win.startMs) / win.msPerColumn) + 1);
    paintSampleDots(ctx, {
      look, startMs, msPerPx, msPerSample: win.msPerColumn, sampleCount: last - first + 1,
      sampleAt: (k) => win.signalHigh[first + k] * win.signalScale, firstSample: win.firstSample + first,
      heightPx, palette, dpr, widthPx,
    });
  }
  ctx.fillStyle = palette.playhead;
  ctx.fillRect(Math.floor(widthPx / 2), 0, Math.max(1, Math.round(dpr)), heightPx);
}

function label(spanMs) {
  return spanMs >= 1000 ? `${(spanMs / 1000).toFixed(spanMs % 1000 ? 2 : 0)} s / screen` : `${spanMs.toFixed(spanMs < 20 ? 1 : 0)} ms / screen`;
}

function row(look, spanMs) {
  const canvas = el('canvas', { style: { width: `${WIDTH_CSS}px`, height: `${HEIGHT_CSS}px`, display: 'block', borderRadius: 'var(--ff-radius-sm, 4px)' } });
  requestAnimationFrame(() => paintView(canvas, look, spanMs));
  return el('div', { class: 'zl-row' }, [el('div', { class: 'zl-label', text: label(spanMs) }), canvas]);
}

/** One canvas the reviewer zooms with the slider or the wheel, from 5 s down to 4 ms per screen, to judge the in-between. */
function liveRow(look) {
  const canvas = el('canvas', { style: { width: `${WIDTH_CSS}px`, height: `${HEIGHT_CSS}px`, display: 'block', borderRadius: 'var(--ff-radius-sm, 4px)' } });
  const caption = el('div', { class: 'zl-label' });
  const slider = el('input', { type: 'range', min: '0', max: '1000', value: '0', style: { width: `${WIDTH_CSS}px` } });
  const spanAt = (v) => MAX_SPAN_MS * Math.pow(MIN_SPAN_MS / MAX_SPAN_MS, v / 1000);
  let frame = 0;
  const draw = () => {
    frame = 0;
    const span = spanAt(Number(slider.value));
    caption.textContent = `live: ${label(span)} (drag the slider or scroll on the waveform)`;
    paintView(canvas, look, span);
  };
  const schedule = () => { if (!frame) frame = requestAnimationFrame(draw); };
  slider.addEventListener('input', schedule);
  canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    e.stopPropagation();
    slider.value = String(Math.max(0, Math.min(1000, Number(slider.value) - e.deltaY * 0.8)));
    schedule();
  }, { passive: false });
  requestAnimationFrame(draw);
  return el('div', { class: 'zl-row zl-row--live' }, [caption, canvas, slider]);
}

const STYLE = `
.zl { display: flex; flex-direction: column; gap: var(--ff-space-3, 12px); padding: var(--ff-space-4, 16px); background: var(--ff-bg, #111); color: var(--ff-text, #eee); font: 12px/1.4 var(--ff-font-sans, system-ui); width: ${WIDTH_CSS + 32}px; box-sizing: border-box; }
.zl h2 { margin: 0; font-size: 15px; }
.zl p { margin: 0; color: var(--ff-text-muted, #aaa); max-width: ${WIDTH_CSS}px; }
.zl-row { display: flex; flex-direction: column; gap: 4px; }
.zl-label { color: var(--ff-text-muted, #aaa); font-variant-numeric: tabular-nums; }
.zl-row--live { padding-top: var(--ff-space-2, 8px); border-top: 1px solid var(--ff-border, #333); }
`;

/** A board: title, the idea, the five fixed zoom levels and the live row. */
export function zoomLookBoard(look, title, idea) {
  return el('div', { class: 'zl', dataset: { look } }, [
    el('style', { text: STYLE }),
    el('h2', { text: title }),
    el('p', { text: idea }),
    ...LEVELS_MS.map((ms) => row(look, ms)),
    liveRow(look),
  ]);
}
