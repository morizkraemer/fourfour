<script module>
  import { bandsFromPeaks } from '../waveform-bands.js';
  import { ink, paletteVersion, wavePalette } from '../waveform-render.js';
  import { drawBandRange } from '../waveform-render.js';
  import { drawLoudness } from '../waveform-simple.js';

  const VARIANTS = {
    mini: { step: 2, gap: 1 },
    strip: { step: 3, gap: 1 },
  };

  const BITMAP_CACHE_LIMIT = 800;
  const DRAWN_KEYS_LIMIT = 20000;
  const PLACEHOLDER_INK_ALPHA = 0.3;
  const STRIP_DECAY = 0.9;

  const bitmapCache = new Map();
  const dataIds = new WeakMap();
  let nextDataId = 1;

  function dataKey(data) {
    if (!data) return '0';
    let id = dataIds.get(data);
    if (!id) {
      id = nextDataId++;
      dataIds.set(data, id);
    }
    return String(id);
  }

  function rememberBitmap(key, bitmap) {
    bitmapCache.set(key, bitmap);
    if (bitmapCache.size > BITMAP_CACHE_LIMIT) bitmapCache.delete(bitmapCache.keys().next().value);
  }

  function recallBitmap(key) {
    const hit = bitmapCache.get(key);
    if (!hit) return null;
    bitmapCache.delete(key);
    bitmapCache.set(key, hit);
    return hit;
  }

  const drawnKeys = new Set();

  function wasDrawnBefore(key) {
    if (drawnKeys.has(key)) return true;
    drawnKeys.add(key);
    if (drawnKeys.size > DRAWN_KEYS_LIMIT) drawnKeys.delete(drawnKeys.values().next().value);
    return false;
  }

  const pendingDraws = new Set();
  let flushHandle = 0;

  function scheduleDraw(run) {
    pendingDraws.add(run);
    if (flushHandle) return;
    flushHandle = requestAnimationFrame(() => {
      flushHandle = 0;
      const batch = [...pendingDraws];
      pendingDraws.clear();
      for (const queued of batch) queued();
    });
  }

  function cancelDraw(run) {
    pendingDraws.delete(run);
  }
</script>

<script>
  /*
    Static preview for list rows (variant "mini") and the detail pane (variant "strip").
    `look` "envelope" (default, rows) is one loudness envelope; "bands" is the 3-band picture of the
    players. `bands` is BandColumns ({low, mid, high}, 0..1); `peaks` (mono 0..1) is the legacy input
    and is turned into a band mix.
  */
  let {
    bands = null,
    peaks = null,
    width = 120,
    height = 14,
    variant = 'mini',
    look = 'envelope',
    cues = [],
    cueStyle = null,
    /** Subtle sine wave on placeholder bars while analysis is in progress. */
    animating = false,
  } = $props();

  let canvas;

  function source() {
    if (bands && bands.low && bands.low.length) return bands;
    return peaks && peaks.length ? peaks : null;
  }

  function bandsOf(data) {
    return data.low ? data : bandsFromPeaks(data);
  }

  function bitmapKey(dpr, data) {
    return `${variant}|${look}|${width}|${height}|${dpr}|${paletteVersion()}|${dataKey(data)}`;
  }

  function paintPlaceholder(ctx, dpr, phase) {
    const cfg = VARIANTS[variant] ?? VARIANTS.mini;
    const stride = cfg.step + cfg.gap;
    const count = Math.max(1, Math.floor(width / stride));
    const mid = height / 2;
    ctx.fillStyle = ink(wavePalette(), PLACEHOLDER_INK_ALPHA);
    for (let i = 0; i < count; i++) {
      let p = 0;
      if (animating) {
        const x = (i / Math.max(1, count - 1)) * Math.PI * 2;
        p = 0.1 + 0.16 * (0.5 + 0.5 * Math.sin(x * 1.4 + phase));
      }
      const barHeight = Math.max(1, (0.12 + p * 0.88) * height);
      ctx.fillRect(Math.round(i * stride * dpr), Math.round((mid - barHeight / 2) * dpr), Math.round(cfg.step * dpr), Math.max(1, Math.round(barHeight * dpr)));
    }
  }

  function paintData(ctx, data, pixelW, pixelH, dpr) {
    const palette = wavePalette();
    if (look === 'bands') {
      drawBandRange(ctx, bandsOf(data), { startFrac: 0, endFrac: 1, widthPx: pixelW, heightPx: pixelH, palette, dpr, decay: STRIP_DECAY });
      return;
    }
    drawLoudness(ctx, bandsOf(data), { widthPx: pixelW, heightPx: pixelH, palette });
  }

  function bitmapFor(data, key, dpr, pixelW, pixelH) {
    const bitmap = document.createElement('canvas');
    bitmap.width = pixelW;
    bitmap.height = pixelH;
    paintData(bitmap.getContext('2d'), data, pixelW, pixelH, dpr);
    rememberBitmap(key, bitmap);
    return bitmap;
  }

  function draw(phase = 0) {
    if (!canvas) return;
    const dpr = window.devicePixelRatio || 1;
    const pixelW = Math.round(width * dpr);
    const pixelH = Math.round(height * dpr);
    if (canvas.width !== pixelW || canvas.height !== pixelH) {
      canvas.width = pixelW;
      canvas.height = pixelH;
    }
    const cssW = `${width}px`;
    const cssH = `${height}px`;
    if (canvas.style.width !== cssW) canvas.style.width = cssW;
    if (canvas.style.height !== cssH) canvas.style.height = cssH;

    const ctx = canvas.getContext('2d');
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, pixelW, pixelH);

    const data = source();
    if (!data) {
      paintPlaceholder(ctx, dpr, phase);
    } else {
      const key = bitmapKey(dpr, data);
      let bitmap = recallBitmap(key);
      if (!bitmap && wasDrawnBefore(key)) bitmap = bitmapFor(data, key, dpr, pixelW, pixelH);
      if (bitmap) ctx.drawImage(bitmap, 0, 0);
      else paintData(ctx, data, pixelW, pixelH, dpr);
    }

    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const markerStyle = cueStyle ?? (variant === 'strip' ? 'line' : 'dot');
    for (const cue of cues || []) {
      const frac = Math.max(0, Math.min(1, cue.position ?? 0));
      const x = Math.round(frac * (width - 1)) + 0.5;
      ctx.fillStyle = cue.color || '#4ade80';
      if (markerStyle === 'line') {
        ctx.fillRect(x - 0.5, 0, 1, height);
        ctx.beginPath();
        ctx.arc(x, 2.5, 2.5, 0, Math.PI * 2);
        ctx.fill();
      } else {
        ctx.beginPath();
        ctx.arc(x, 3, 2.5, 0, Math.PI * 2);
        ctx.fill();
      }
    }
  }

  $effect(() => {
    bands;
    peaks;
    width;
    height;
    variant;
    look;
    cues;
    cueStyle;
    animating;

    const data = source();
    if (animating && !data) {
      let phase = 0;
      let live = true;
      const tick = () => {
        if (!live) return;
        phase += 0.11;
        draw(phase);
        scheduleDraw(tick);
      };
      scheduleDraw(tick);
      return () => {
        live = false;
        cancelDraw(tick);
      };
    }

    const run = () => draw(0);
    if (data && recallBitmap(bitmapKey(window.devicePixelRatio || 1, data))) {
      run();
      return;
    }
    scheduleDraw(run);
    return () => cancelDraw(run);
  });
</script>

<canvas bind:this={canvas} class="ff-waveform ff-waveform--{variant}"></canvas>
