<script>
  import { WaveformRenderer } from '../waveform-renderer.ts';

  let {
    colorData = null,
    previewBytes = null,
    progress = 0,
    cues = [],
    beats = [],
    durationMs = 0,
    trackKey = null,
    onScrub = undefined,
    followPlayhead = true,
  } = $props();

  let canvas;
  let renderer;
  let ro;

  /** @type {import('../waveform-renderer.ts').WaveformRenderer | null} */
  export function getRenderer() {
    return renderer;
  }

  $effect(() => {
    if (!canvas) return;
    renderer?.destroy();
    renderer = new WaveformRenderer(canvas, { onScrub, followPlayhead });
    ro = new ResizeObserver(() => renderer?.resize());
    ro.observe(canvas.parentElement ?? canvas);

    return () => {
      ro?.disconnect();
      renderer?.destroy();
      renderer = null;
    };
  });

  $effect(() => {
    trackKey;
    renderer?.resetView();
  });

  $effect(() => {
    if (!renderer) return;
    colorData;
    previewBytes;
    progress;
    cues;
    beats;
    durationMs;
    followPlayhead;
    onScrub;
    renderer.setFollowPlayhead(followPlayhead);
    renderer.setColorData(colorData);
    renderer.setPreviewBytes(previewBytes);
    renderer.setProgress(progress);
    renderer.setCues(cues);
    renderer.setBeats(beats);
    renderer.setDurationMs(durationMs);
  });

  function onKeydown(e) {
    if (renderer?.handleKey(e)) {
      e.stopPropagation();
    }
  }
</script>

<canvas
  bind:this={canvas}
  class="ff-waveform-strip"
  tabindex="0"
  role="slider"
  aria-label="Track waveform"
  aria-valuenow={Math.round(progress * 100)}
  aria-valuemin="0"
  aria-valuemax="100"
  onkeydown={onKeydown}
></canvas>

<style>
  .ff-waveform-strip {
    display: block;
    width: 100%;
    height: 100%;
    cursor: pointer;
    outline: none;
  }

  .ff-waveform-strip:focus-visible {
    box-shadow: inset 0 0 0 1px var(--ff-accent);
  }
</style>
