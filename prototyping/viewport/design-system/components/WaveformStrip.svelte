<script>
  import { WaveformRenderer } from '../waveform-renderer.ts';

  let {
    colorData = null,
    previewBytes = null,
    progress = 0,
    cues = [],
    onScrub = undefined,
  } = $props();

  let canvas;
  let renderer;
  let ro;

  $effect(() => {
    if (!canvas) return;
    renderer?.destroy();
    renderer = new WaveformRenderer(canvas, { onScrub });
    ro = new ResizeObserver(() => renderer?.resize());
    ro.observe(canvas.parentElement ?? canvas);

    return () => {
      ro?.disconnect();
      renderer?.destroy();
      renderer = null;
    };
  });

  $effect(() => {
    if (!renderer) return;
    colorData;
    previewBytes;
    progress;
    cues;
    onScrub;
    renderer.setColorData(colorData);
    renderer.setPreviewBytes(previewBytes);
    renderer.setProgress(progress);
    renderer.setCues(cues);
  });
</script>

<canvas bind:this={canvas} class="ff-waveform-strip" aria-hidden="true"></canvas>

<style>
  .ff-waveform-strip {
    display: block;
    width: 100%;
    height: 100%;
    cursor: pointer;
  }
</style>
