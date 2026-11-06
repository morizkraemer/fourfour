<script>
  /*
    WaveformView: shared wrapper around the WaveformDisplay engine, used by both
    players so the 3-band look, cues and beat grid match exactly.

      mode="compact"  full-track view, no zoom, click/drag scrubs.
      mode="expanded" overview + playhead-locked zoom view.

    Waveform data is BandColumns ({low, mid, high}, 0..1 per column, see
    waveform-bands.js). It is pushed via setData only when the track or its
    waveform changes (setData resets zoom); the playhead follows every tick.
  */
  import WaveformDisplay from '../waveform-display.js';
  import Waveform from './Waveform.svelte';

  let {
    mode = 'compact',
    bands = null,
    bandsDetail = null,
    bandsOverview = null,
    bandsZoom = null,
    detailPending = false,
    windowSource = null,
    beats = [],
    cues = [],
    bpm = 0,
    progress = 0,
    playing = false,
    durationMs = 0,
    trackKey = null,
    analyzing = false,
    emptyLabel = null,
    onSeek = undefined,
    onCueMove = undefined,
    cueSnap = null,
  } = $props();

  let placeholderWidth = $state(0);
  let placeholderHeight = $state(0);

  let container;
  let display = null;
  let lastDataKey = null;

  function markerData() {
    return (cues ?? []).map((c) => ({ position: c.position, color: c.color, label: c.label, key: c.key, padded: c.padded }));
  }

  function buildData() {
    return {
      bands: (mode === 'expanded' ? (bandsDetail ?? bands) : bands) ?? undefined,
      bandsZoom: (mode === 'expanded' ? bandsZoom : null) ?? undefined,
      bandsOverview: (bandsOverview ?? (mode === 'expanded' ? bands : null)) ?? undefined,
      beats: beats ?? [],
      cues: markerData(),
      duration_ms: durationMs ?? 0,
      bpm: Number(bpm) || 0,
    };
  }

  export function zoomIn() { display?.zoomIn(); }
  export function zoomOut() { display?.zoomOut(); }
  export function resetZoom() { display?.resetZoom(); }
  export function msPerPixel() { return display?.msPerPixel() ?? 0; }
  export function msPerSample() { return display?.msPerSample(); }

  $effect(() => {
    if (!container) return;
    display = new WaveformDisplay(container, { mode, onSeek, onCueMove, paint: 'frame' });
    lastDataKey = null;
    return () => {
      display?.destroy();
      display = null;
    };
  });

  $effect(() => {
    if (display) display.onSeek = onSeek;
  });

  $effect(() => {
    if (display) display.onCueMove = onCueMove;
  });

  $effect(() => {
    display?.setCueSnap(cueSnap);
  });

  $effect(() => {
    const detailLen = (mode === 'expanded' ? (bandsDetail ?? bands) : bands)?.low.length ?? 0;
    const key = `${trackKey}:${mode}:${detailLen}:${bandsOverview?.low.length ?? 0}:${durationMs}`;
    if (!display) return;
    if (key === lastDataKey) return;
    lastDataKey = key;
    display.setData(buildData(), progress);
  });

  $effect(() => {
    if (mode === 'expanded') display?.setDetailBands(bandsZoom ?? undefined);
  });

  $effect(() => {
    if (mode === 'expanded') display?.setDetailPending(detailPending);
  });

  $effect(() => {
    if (mode === 'expanded') display?.setWindowSource(windowSource);
  });

  $effect(() => {
    display?.setCues(markerData());
  });

  $effect(() => {
    display?.setBeats(beats ?? []);
  });

  $effect(() => {
    display?.setTransport(playing, progress);
  });
</script>

<div class="ff-waveform-view" bind:this={container}>
  {#if mode === 'expanded' && !bands && emptyLabel}
    <div class="ff-waveform-view__empty" class:ff-waveform-view__empty--analyzing={analyzing}>
      <div class="ff-waveform-view__empty-strip" bind:clientWidth={placeholderWidth} bind:clientHeight={placeholderHeight}>
        <Waveform width={placeholderWidth || 300} height={placeholderHeight || 48} variant="strip" look="bands" animating={analyzing} />
      </div>
      <div class="ff-waveform-view__empty-body">
        <span class="ff-waveform-view__empty-line"></span>
        <span class="ff-waveform-view__empty-label">{emptyLabel}</span>
      </div>
    </div>
  {:else if analyzing && !bands}
    <div class="ff-waveform-view__analyzing" bind:clientWidth={placeholderWidth} bind:clientHeight={placeholderHeight}>
      <Waveform width={placeholderWidth || 300} height={placeholderHeight || 48} variant="strip" look="bands" animating />
    </div>
  {/if}
</div>

<style>
  .ff-waveform-view {
    position: relative;
    width: 100%;
    height: 100%;
    min-height: 0;
  }

  .ff-waveform-view__analyzing {
    position: absolute;
    inset: 0;
    z-index: 1;
    display: flex;
    align-items: center;
    background: var(--ff-wave-bg, #0d0d0d);
    pointer-events: none;
  }

  .ff-waveform-view__empty {
    position: absolute;
    inset: 0;
    z-index: 1;
    display: flex;
    flex-direction: column;
    background: var(--ff-wave-bg, #0d0d0d);
    pointer-events: none;
  }

  .ff-waveform-view__empty-strip {
    display: flex;
    align-items: center;
    height: 48px;
    flex-shrink: 0;
    border-bottom: 1px solid rgba(var(--ff-wave-ink, 255, 255, 255), 0.08);
  }

  .ff-waveform-view__empty-body {
    position: relative;
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .ff-waveform-view__empty-line {
    position: absolute;
    left: 0;
    right: 0;
    top: 50%;
    height: 1px;
    background: rgba(var(--ff-wave-ink, 255, 255, 255), 0.08);
  }

  .ff-waveform-view__empty-label {
    position: relative;
    padding: 4px 12px;
    background: var(--ff-wave-bg, #0d0d0d);
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-label-sm);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--ff-muted);
  }

  .ff-waveform-view__empty--analyzing .ff-waveform-view__empty-label {
    animation: ff-waveform-view-empty-pulse 1.1s ease-in-out infinite alternate;
  }

  @keyframes ff-waveform-view-empty-pulse {
    from { opacity: 1; }
    to { opacity: 0.55; }
  }

  @media (prefers-reduced-motion: reduce) {
    .ff-waveform-view__empty--analyzing .ff-waveform-view__empty-label {
      animation: none;
    }
  }

  :global(.ff-waveform-view .waveform-display) {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    background: var(--ff-wave-bg, #0d0d0d);
    overflow: hidden;
  }
  :global(.ff-waveform-view .waveform-display__overview) {
    display: block;
    width: 100%;
    height: 48px;
    cursor: pointer;
    flex-shrink: 0;
    border-bottom: 1px solid rgba(var(--ff-wave-ink, 255, 255, 255), 0.08);
  }
  :global(.ff-waveform-view .waveform-display__zoom) {
    display: block;
    width: 100%;
    height: 100%;
  }
</style>
