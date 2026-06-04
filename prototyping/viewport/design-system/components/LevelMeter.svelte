<script>
  // Master level meter (Svelte port of level-meter.js). Segmented stereo VU for status bar.
  import './level-meter.css';

  let { left = 0, right = 0, segments = 16, label = 'MASTER' } = $props();

  function zone(i) {
    const frac = i / segments;
    if (frac >= 0.85) return 'danger';
    if (frac >= 0.65) return 'warn';
    return 'green';
  }

  let litLeft = $derived(Math.round(Math.max(0, Math.min(1, left)) * segments));
  let litRight = $derived(Math.round(Math.max(0, Math.min(1, right)) * segments));
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
