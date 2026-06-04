<script module>
  // Shared draw helpers (Svelte port of waveform.js). Canvas-2D faux/real peak
  // bars for the track-row preview and player strip. Per ui-architecture.md the
  // real waveform is Canvas, not DOM — this is that primitive.
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
      env += (rand() - 0.5) * 0.4;
      env = Math.max(0.15, Math.min(1, env));
      out[i] = env * (0.55 + rand() * 0.45);
    }
    return out;
  }

  function rootColor(name, fallback) {
    const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    return v || fallback;
  }

  const VARIANTS = {
    mini: { step: 2, gap: 1, base: '--ff-muted', played: '--ff-text-mid', floor: 0.12 },
    strip: { step: 3, gap: 1, base: '--ff-text-mid', played: '--ff-accent', floor: 0.06 },
  };
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
  } = $props();

  let canvas;

  function draw() {
    if (!canvas) return;
    const cfg = VARIANTS[variant] ?? VARIANTS.mini;
    const markerStyle = cueStyle ?? (variant === 'strip' ? 'line' : 'dot');
    const w = width;
    const h = height;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
    canvas.style.width = `${w}px`;
    canvas.style.height = `${h}px`;
    const ctx = canvas.getContext('2d');
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);

    const stride = cfg.step + cfg.gap;
    const bars = Math.max(1, Math.floor(w / stride));
    const data = peaks && peaks.length ? peaks : fauxPeaks(bars, seed);
    const baseColor = rootColor(cfg.base, '#6a6a6a');
    const playedColor = rootColor(cfg.played, '#aaaaaa');
    const playedBars = Math.round(bars * Math.max(0, Math.min(1, progress)));
    const mid = h / 2;

    for (let i = 0; i < bars; i++) {
      const p = data[Math.floor((i / bars) * data.length)] ?? cfg.floor;
      const bh = Math.max(1, (cfg.floor + p * (1 - cfg.floor)) * h);
      ctx.fillStyle = i < playedBars ? playedColor : baseColor;
      ctx.fillRect(i * stride, mid - bh / 2, cfg.step, bh);
    }

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

  // Redraw whenever the reactive inputs change; the read of each prop inside
  // draw() registers the dependency. rAF on first paint so token colors resolve.
  $effect(() => {
    peaks; width; height; variant; seed; progress; cues; cueStyle;
    requestAnimationFrame(draw);
  });
</script>

<canvas bind:this={canvas} class="ff-waveform ff-waveform--{variant}"></canvas>
