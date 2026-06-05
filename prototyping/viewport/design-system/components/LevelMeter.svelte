<script>
  // Master level meter (Svelte port of level-meter.js). Segmented stereo VU for status bar.
  import './level-meter.css';

  let { left = 0, right = 0, segments = 16, label = 'MASTER' } = $props();

  /** Ignore sub-threshold bleed so idle meters stay dark. */
  const LEVEL_FLOOR = 0.06;

  function zone(i) {
    const frac = i / segments;
    if (frac >= 0.85) return 'danger';
    if (frac >= 0.65) return 'warn';
    return 'green';
  }

  function litCount(level) {
    const v = Math.max(0, Math.min(1, level));
    if (v < LEVEL_FLOOR) return 0;
    return Math.round(v * segments);
  }

  let litLeft = $derived(litCount(left));
  let litRight = $derived(litCount(right));
</script>

<div class="ff-meter">
  {#if label}
    <span class="ff-meter__label">{label}</span>
  {/if}
  <div class="ff-meter__channels">
    <div class="ff-meter__channel" aria-hidden="true">
      {#each { length: segments } as _, i (i)}
        {@const on = i < litLeft}
        <span
          class={[
            'ff-meter__seg',
            on && 'ff-meter__seg--on',
            on && `ff-meter__seg--${zone(i)}`,
          ]
            .filter(Boolean)
            .join(' ')}
        ></span>
      {/each}
    </div>
    <div class="ff-meter__channel" aria-hidden="true">
      {#each { length: segments } as _, i (i)}
        {@const on = i < litRight}
        <span
          class={[
            'ff-meter__seg',
            on && 'ff-meter__seg--on',
            on && `ff-meter__seg--${zone(i)}`,
          ]
            .filter(Boolean)
            .join(' ')}
        ></span>
      {/each}
    </div>
  </div>
</div>
