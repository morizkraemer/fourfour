<script>
  import { Waveform } from '../../../../viewport/design-system/svelte.js';
  import { ALL_CUES, BANDS_WIDE, BANDS_ZOOM } from './w2-data.js';

  let { width = 356, height = 64, selectedKey = null, grid = true, tip = '', playhead = 0.45, zoom = false, window: win = false } = $props();
  const TOTAL_MS = 491000;
  const step = $derived(zoom ? width / 16 : 7);
  const beats = $derived(Array.from({ length: Math.floor(width / step) + 1 }, (_, i) => i));
  const cues = $derived(zoom ? ALL_CUES.filter((c) => c.key === selectedKey) : ALL_CUES);
</script>

<div class="w" style="width:{width}px;height:{height}px">
  <Waveform bands={zoom ? BANDS_ZOOM : BANDS_WIDE} {width} {height} variant="strip" look="bands" />
  {#if grid}
    <div class="w__grid">
      {#each beats as i}<i class:w__bar={i % 4 === 0} style="left:{Math.round(i * step)}px"></i>{/each}
    </div>
  {/if}
  {#each cues as c (c.key)}
    {@const left = zoom ? Math.round(width * 0.5) : Math.round((c.timeMs / TOTAL_MS) * width)}
    <div class="w__cue" class:w__cue--on={c.key === selectedKey} style="left:{left}px;--c:{c.color}">
      <span class="w__flag">{c.letter}</span>
      {#if c.key === selectedKey && tip}<span class="w__tip">{tip}</span>{/if}
    </div>
  {/each}
  {#if win}<div class="w__win" style="left:{Math.round(width * 0.47) - 12}px"></div>{/if}
  {#if !zoom}<div class="w__play" style="left:{Math.round(playhead * width)}px"></div>{/if}
  {#if zoom}<span class="w__zl">16 beats</span>{/if}
</div>

<style>
  .w { position: relative; border-radius: var(--ff-radius-md); overflow: visible; background: var(--ff-wave-bg); }
  .w__grid { position: absolute; inset: 0; pointer-events: none; overflow: hidden; }
  .w__grid i { position: absolute; top: 0; bottom: 0; width: 1px; background: rgba(255, 255, 255, 0.07); }
  .w__grid i.w__bar { background: rgba(255, 255, 255, 0.18); }
  .w__cue { position: absolute; top: 0; bottom: 0; width: 1px; background: var(--c); }
  .w__flag { position: absolute; top: 0; left: 0; min-width: 12px; height: 13px; padding: 0 3px; box-sizing: border-box; background: var(--c); color: #0a0a0a; font-family: var(--ff-font-mono); font-size: 9.5px; font-weight: 700; display: flex; align-items: center; justify-content: center; border-radius: 0 2px 2px 0; }
  .w__cue--on { width: 2px; margin-left: -0.5px; box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.35); z-index: 3; }
  .w__cue--on .w__flag { height: 16px; min-width: 15px; font-size: 11px; }
  .w__tip { position: absolute; bottom: calc(100% + 6px); left: -40px; padding: 3px 7px; border-radius: 4px; white-space: nowrap; background: var(--ff-accent); color: #0a0a0a; font-family: var(--ff-font-mono); font-size: 10.5px; font-weight: 600; z-index: 5; }
  .w__win { position: absolute; top: -2px; bottom: -2px; width: 24px; border: 1px solid rgba(255, 255, 255, 0.5); border-radius: 2px; background: rgba(255, 255, 255, 0.08); pointer-events: none; z-index: 2; }
  .w__zl { position: absolute; right: 6px; bottom: 4px; font-family: var(--ff-font-mono); font-size: 9.5px; color: var(--ff-text-mid); background: rgba(0, 0, 0, 0.55); padding: 1px 5px; border-radius: 2px; z-index: 2; }
  .w__play { position: absolute; top: -3px; bottom: -3px; width: 2px; margin-left: -1px; background: var(--ff-wave-playhead); z-index: 4; }
</style>
