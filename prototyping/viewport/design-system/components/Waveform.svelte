<script module>
  const VARIANTS = {
    mini: { step: 2, gap: 1, base: '--ff-muted', played: '--ff-text-mid', floor: 0.12 },
    strip: { step: 3, gap: 1, base: '--ff-text-mid', played: '--ff-accent', floor: 0.06 },
  };

  const BASE_FALLBACK = '#6a6a6a';
  const PLAYED_FALLBACK = '#aaaaaa';
  const BITMAP_CACHE_LIMIT = 800;
  const DRAWN_KEYS_LIMIT = 20000;

  const tokenColors = new Map();
  let tokenVersion = 0;

  function invalidateTokens() {
    tokenColors.clear();
    tokenVersion++;
  }

  if (typeof document !== 'undefined') {
    new MutationObserver(invalidateTokens).observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['class', 'style', 'data-theme'],
    });
    window.matchMedia?.('(prefers-color-scheme: dark)').addEventListener?.('change', invalidateTokens);
  }

  function resolvedToken(name) {
    const hit = tokenColors.get(name);
    if (hit) return hit;
    const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    if (v) tokenColors.set(name, v);
    return v || null;
  }

  const bitmapCache = new Map();
  const peaksIds = new WeakMap();
  let nextPeaksId = 1;

  function peaksKey(peaks) {
    if (!peaks || !peaks.length) return '0';
    let id = peaksIds.get(peaks);
    if (!id) {
      id = nextPeaksId++;
      peaksIds.set(peaks, id);
    }
    return `${id}:${peaks.length}`;
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

  function tokensResolved(cfg) {
    return !!(resolvedToken(cfg.base) && resolvedToken(cfg.played));
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
  let {
    peaks = null,
    width = 120,
    height = 14,
    variant = 'mini',
    seed = 1,
    progress = 0,
    cues = [],
    cueStyle = null,
    /** Subtle sine wave on placeholder bars while analysis is in progress. */
    animating = false,
  } = $props();

  let canvas;

  function layout() {
    const cfg = VARIANTS[variant] ?? VARIANTS.mini;
    const stride = cfg.step + cfg.gap;
    const bars = Math.max(1, Math.floor(width / stride));
    const playedBars = Math.round(bars * Math.max(0, Math.min(1, progress)));
    const dpr = window.devicePixelRatio || 1;
    return { cfg, stride, bars, playedBars, dpr };
  }

  function bitmapKey({ playedBars, dpr }) {
    return `${variant}|${width}|${height}|${dpr}|${tokenVersion}|${peaksKey(peaks)}|${playedBars}`;
  }

  function paintBars(ctx, { cfg, stride, bars, playedBars }, phase) {
    const w = width;
    const h = height;
    const data = peaks && peaks.length ? peaks : null;
    const baseColor = resolvedToken(cfg.base) ?? BASE_FALLBACK;
    const playedColor = resolvedToken(cfg.played) ?? PLAYED_FALLBACK;
    const mid = h / 2;

    for (let i = 0; i < bars; i++) {
      let p;
      if (data) {
        p = data[Math.floor((i / bars) * data.length)] ?? cfg.floor;
      } else if (animating) {
        const x = (i / Math.max(1, bars - 1)) * Math.PI * 2;
        p = 0.1 + 0.16 * (0.5 + 0.5 * Math.sin(x * 1.4 + phase));
      } else {
        p = 0;
      }
      const bh = Math.max(1, (cfg.floor + p * (1 - cfg.floor)) * h);
      ctx.fillStyle = i < playedBars ? playedColor : baseColor;
      ctx.fillRect(i * stride, mid - bh / 2, cfg.step, bh);
    }
  }

  function bitmapFor(geometry, key, pixelW, pixelH) {
    const bitmap = document.createElement('canvas');
    bitmap.width = pixelW;
    bitmap.height = pixelH;
    const bitmapCtx = bitmap.getContext('2d');
    bitmapCtx.setTransform(geometry.dpr, 0, 0, geometry.dpr, 0, 0);
    paintBars(bitmapCtx, geometry, 0);
    rememberBitmap(key, bitmap);
    return bitmap;
  }

  function draw(phase = 0) {
    if (!canvas) return;
    const geometry = layout();
    const { dpr } = geometry;
    const w = width;
    const h = height;
    const pixelW = Math.round(w * dpr);
    const pixelH = Math.round(h * dpr);
    if (canvas.width !== pixelW || canvas.height !== pixelH) {
      canvas.width = pixelW;
      canvas.height = pixelH;
    }
    const cssW = `${w}px`;
    const cssH = `${h}px`;
    if (canvas.style.width !== cssW) canvas.style.width = cssW;
    if (canvas.style.height !== cssH) canvas.style.height = cssH;

    const ctx = canvas.getContext('2d');
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, pixelW, pixelH);

    const placeholderFrame = animating && !(peaks && peaks.length);
    const key = bitmapKey(geometry);
    let bitmap = placeholderFrame ? null : recallBitmap(key);
    if (!bitmap && !placeholderFrame && tokensResolved(geometry.cfg) && wasDrawnBefore(key)) {
      bitmap = bitmapFor(geometry, key, pixelW, pixelH);
    }
    if (bitmap) {
      ctx.drawImage(bitmap, 0, 0);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    } else {
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      paintBars(ctx, geometry, phase);
    }

    const markerStyle = cueStyle ?? (variant === 'strip' ? 'line' : 'dot');
    for (const cue of cues || []) {
      const frac = Math.max(0, Math.min(1, cue.position ?? 0));
      const x = Math.round(frac * (w - 1)) + 0.5;
      ctx.fillStyle = cue.color || '#4ade80';
      if (markerStyle === 'line') {
        ctx.fillRect(x - 0.5, 0, 1, h);
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
    peaks;
    width;
    height;
    variant;
    seed;
    progress;
    cues;
    cueStyle;
    animating;

    if (animating && !(peaks && peaks.length)) {
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
    if (recallBitmap(bitmapKey(layout()))) {
      run();
      return;
    }
    scheduleDraw(run);
    return () => cancelDraw(run);
  });
</script>

<canvas bind:this={canvas} class="ff-waveform ff-waveform--{variant}"></canvas>
