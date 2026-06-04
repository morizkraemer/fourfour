<script>
  /*
    WaveformView — shared wrapper around the WaveformDisplay engine, used by
    both players so the 3-band look, cues and beat grid match exactly.

      mode="compact"  — static full-track view, no zoom, click/drag scrubs.
      mode="expanded" — overview + Rekordbox playhead-locked zoom view.

    The store keeps rgb in 0–1; WaveformDisplay expects 0–255, so the scale
    happens here (one place). Data is pushed via setData only when the track or
    its waveform changes (setData resets zoom); the playhead follows every tick.
  */
  import WaveformDisplay from '../waveform-display.js';

  let {
    mode = 'compact',
    // Compact uses colorData (full track). Expanded prefers the hi-res detail
    // for the zoom view + the fixed overview for the preview bar.
    colorData = null,
    colorDetail = null,
    colorOverview = null,
    previewBytes = null,
    beats = [],
    cues = [],
    bpm = 0,
    progress = 0,
    playing = false,
    durationMs = 0,
    trackKey = null,
    onSeek = undefined,
  } = $props();

  let container;
  let display = null;
  let ro;
  let lastDataKey = null;

  // WaveformDisplay's bandAmps divides weights by 255; the store keeps rgb 0–1.
  function toDisplay(samples) {
    if (!samples) return undefined;
    return samples.map((s) => ({ amp: s.amp, r: s.r * 255, g: s.g * 255, b: s.b * 255 }));
  }

  function buildData() {
    const detail = mode === 'expanded' ? (colorDetail ?? colorData) : colorData;
    const overview = mode === 'expanded' ? (colorOverview ?? colorData) : null;
    return {
      waveform_color: toDisplay(detail) ?? [],
      waveform_overview: toDisplay(overview),
      waveform_preview: previewBytes ?? undefined,
      beats: beats ?? [],
      cues: (cues ?? []).map((c) => ({ position: c.position, color: c.color })),
      duration_ms: durationMs ?? 0,
      bpm: Number(bpm) || 0,
    };
  }

  // Public zoom controls for the expanded toolbar.
  export function zoomIn() { display?.zoomIn(); }
  export function zoomOut() { display?.zoomOut(); }
  export function resetZoom() { display?.resetZoom(); }

  $effect(() => {
    if (!container) return;
    display = new WaveformDisplay(container, { mode, onSeek });
    ro = new ResizeObserver(() => display?.redraw());
    ro.observe(container);
    lastDataKey = null;
    return () => {
      ro?.disconnect();
      display?.destroy();
      display = null;
    };
  });

  // Keep the seek callback live without recreating the engine.
  $effect(() => {
    if (display) display.onSeek = onSeek;
  });

  // Push data only when the track or its waveform changes — NOT on every tick.
  $effect(() => {
    const detailLen = (mode === 'expanded' ? (colorDetail ?? colorData) : colorData)?.length ?? 0;
    // beats?.[0]?.time_ms captures beat-grid edits (shift/set) that move the grid
    // without changing its length, so the engine re-reads the shifted beats.
    const key = `${trackKey}:${mode}:${detailLen}:${colorOverview?.length ?? 0}:${beats?.length ?? 0}:${beats?.[0]?.time_ms ?? 0}:${cues?.length ?? 0}:${durationMs}`;
    if (!display) return;
    if (key === lastDataKey) return;
    lastDataKey = key;
    display.setData(buildData());
  });

  // Tell the engine when playback is running so it interpolates the playhead
  // (the transport only reports position at ~20Hz).
  $effect(() => {
    display?.setPlaying(playing);
  });

  // Playhead follows playback progress (cheap overlay / scroll update).
  $effect(() => {
    display?.setPlayhead(progress);
  });
</script>

<div class="ff-waveform-view" bind:this={container}></div>

<style>
  .ff-waveform-view {
    position: relative;
    width: 100%;
    height: 100%;
    min-height: 0;
  }

  /* WaveformDisplay builds its own DOM (unscoped) — style globally. */
  :global(.ff-waveform-view .waveform-display) {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    background: var(--ff-bg, #0d0d0d);
    overflow: hidden;
  }
  :global(.ff-waveform-view .waveform-display__overview) {
    display: block;
    width: 100%;
    height: 48px;
    cursor: pointer;
    flex-shrink: 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }
  :global(.ff-waveform-view .waveform-display__zoom) {
    display: block;
    width: 100%;
    height: 100%;
  }
</style>
