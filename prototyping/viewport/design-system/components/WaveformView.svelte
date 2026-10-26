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

  let {
    mode = 'compact',
    bands = null,
    bandsDetail = null,
    bandsOverview = null,
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

  function markerData() {
    return (cues ?? []).map((c) => ({ position: c.position, color: c.color, label: c.label }));
  }

  function buildData() {
    return {
      bands: (mode === 'expanded' ? (bandsDetail ?? bands) : bands) ?? undefined,
      bandsOverview: (bandsOverview ?? (mode === 'expanded' ? bands : null)) ?? undefined,
      beats: beats ?? [],
      cues: markerData(),
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
    const detailLen = (mode === 'expanded' ? (bandsDetail ?? bands) : bands)?.low.length ?? 0;
    const key = `${trackKey}:${mode}:${detailLen}:${bandsOverview?.low.length ?? 0}:${beats?.length ? 1 : 0}:${durationMs}`;
    if (!display) return;
    if (key === lastDataKey) return;
    lastDataKey = key;
    // Pass playhead with data so zoom offset centres atomically on load (progress
    // often stays 0 across track changes, so a separate effect would not re-run).
    display.setData(buildData(), progress);
  });

  // A cue edit redraws the markers only: no data reset, so zoom and scroll stay.
  $effect(() => {
    display?.setCues(markerData());
  });

  // A beat-grid edit moves the grid only: no data reset, so zoom and scroll stay.
  $effect(() => {
    display?.setBeats(beats ?? []);
  });

  // Atomic transport update — avoids play/pause ordering jitter between separate
  // playing and progress effects (the transport only reports position at ~20Hz).
  $effect(() => {
    display?.setTransport(playing, progress);
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
