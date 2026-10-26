import './waveform.css';
import { el } from '../utils/dom.js';
import { bandsFromPeaks } from '../waveform-bands.js';
import { drawBandRange, wavePalette } from '../waveform-render.js';

/*
 * Waveform (§ track-row preview + player strip). A Canvas-2D renderer that draws
 * vertical peak bars. Per ui-architecture.md the real waveform is Canvas, not DOM —
 * this is that primitive; for now it draws faux peaks deterministically from a seed
 * when no real `peaks` array is supplied.
 *
 *   createWaveform({ bands?, peaks?, width, height, variant, seed, cues, cueStyle }) → { element, update }
 *     variant: 'mini' (track row preview) | 'strip' (player) — drives envelope smoothing
 *     bands:   optional {low, mid, high} 0..1 columns (waveform-bands.js)
 *     peaks:   optional number[] in 0..1 (mono); omitted → faux peaks from `seed`
 *     cues:    optional [{ position: 0..1, color }] hot-cue markers
 *     cueStyle: 'dot' (colored dot) | 'line' (playhead-style line + dot);
 *               defaults to 'line' for the strip variant, 'dot' otherwise
 */
function mulberry32(seed) {
  let a = seed >>> 0;
  return function () {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function fauxPeaks(count, seed) {
  const rand = mulberry32(seed);
  const out = new Array(count);
  let env = 0.5;
  for (let i = 0; i < count; i++) {
    // smooth-ish envelope so it reads like audio, not noise
    env += (rand() - 0.5) * 0.4;
    env = Math.max(0.15, Math.min(1, env));
    out[i] = env * (0.55 + rand() * 0.45);
  }
  return out;
}

const VARIANTS = {
  mini: { decay: 0.8 },
  strip: { decay: 0.88 },
};

export function createWaveform({
  bands = null,
  peaks = null,
  width = 120,
  height = 14,
  variant = 'mini',
  seed = 1,
  cues = [],
  cueStyle = null,
  animating = false,
} = {}) {
  const cfg = VARIANTS[variant] ?? VARIANTS.mini;
  const markerStyle = cueStyle ?? (variant === 'strip' ? 'line' : 'dot');
  const canvas = el('canvas', { class: `ff-waveform ff-waveform--${variant}` });
  const state = { bands, peaks, width, height, cues, animating, phase: 0 };
  let animRaf = 0;

  function stopAnimLoop() {
    if (animRaf) cancelAnimationFrame(animRaf);
    animRaf = 0;
  }

  function startAnimLoop() {
    stopAnimLoop();
    const tick = () => {
      state.phase += 0.11;
      draw();
      animRaf = requestAnimationFrame(tick);
    };
    animRaf = requestAnimationFrame(tick);
  }

  function draw() {
    const w = state.width;
    const h = state.height;
    const dpr = window.devicePixelRatio || 1;
    const widthPx = Math.round(w * dpr);
    const heightPx = Math.round(h * dpr);
    canvas.width = widthPx;
    canvas.height = heightPx;
    canvas.style.width = `${w}px`;
    canvas.style.height = `${h}px`;
    const ctx = canvas.getContext('2d');
    ctx.setTransform(1, 0, 0, 1, 0, 0);

    const hasPeaks = state.peaks && state.peaks.length;
    let data = state.bands;
    if (!data && hasPeaks) data = bandsFromPeaks(state.peaks);
    if (!data && state.animating) {
      const count = Math.max(8, Math.floor(w / 3));
      data = bandsFromPeaks(Array.from({ length: count }, (_, i) => 0.1 + 0.16 * (0.5 + 0.5 * Math.sin((i / count) * Math.PI * 2 * 1.4 + state.phase))));
    }
    if (data) {
      drawBandRange(ctx, data, {
        startFrac: 0, endFrac: 1, widthPx, heightPx, palette: wavePalette(), dpr, decay: cfg.decay,
      });
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    // Hot-cue markers (drawn on top of the bars)
    const cueList = state.cues || [];
    for (const cue of cueList) {
      const frac = Math.max(0, Math.min(1, cue.position ?? 0));
      const x = Math.round(frac * (w - 1)) + 0.5;
      const color = cue.color || '#4ade80';
      ctx.fillStyle = color;
      if (markerStyle === 'line') {
        ctx.fillRect(x - 0.5, 0, 1, h);          // playhead-style vertical line
        ctx.beginPath();
        ctx.arc(x, 2.5, 2.5, 0, Math.PI * 2);     // dot cap at the top
        ctx.fill();
      } else {
        ctx.beginPath();
        ctx.arc(x, 3, 2.5, 0, Math.PI * 2);       // just a colored dot
        ctx.fill();
      }
    }
  }

  function update(next = {}) {
    Object.assign(state, next);
    const shouldAnimate = state.animating && !(state.peaks && state.peaks.length);
    if (shouldAnimate) startAnimLoop();
    else {
      stopAnimLoop();
      draw();
    }
  }

  if (animating && !(peaks && peaks.length)) startAnimLoop();
  else requestAnimationFrame(draw);

  return {
    element: canvas,
    update,
    destroy() {
      stopAnimLoop();
    },
  };
}
