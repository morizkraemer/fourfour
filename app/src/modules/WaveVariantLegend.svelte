<script>
  import { WAVE_VARIANTS, WAVE_VARIANT_KEYS, currentWaveVariant, onWaveVariant, setWaveVariant, waveSwitchActive } from '$ds/waveform-scaling.js';

  const active = waveSwitchActive();
  let selected = $state(currentWaveVariant());

  $effect(() => onWaveVariant((key) => {
    selected = key;
    const url = new URL(location.href);
    url.searchParams.set('wave', key);
    history.replaceState(null, '', url);
  }));

  function onKey(event) {
    if (!active || event.metaKey || event.ctrlKey || event.altKey) return;
    const target = event.target;
    if (target instanceof HTMLElement && (target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName))) return;
    const key = WAVE_VARIANT_KEYS[Number(event.key) - 1];
    if (key) setWaveVariant(key);
  }
</script>

<svelte:window onkeydown={onKey} />

{#if active}
  <div class="wave-legend" data-wave-legend>
    <div class="wave-legend__title">wave: {selected}</div>
    {#each WAVE_VARIANT_KEYS as key, i}
      <button class="wave-legend__row" class:wave-legend__row--on={key === selected} onclick={() => setWaveVariant(key)}>
        <kbd>{i + 1}</kbd><b>{key}</b> {WAVE_VARIANTS[key].label}
      </button>
    {/each}
  </div>
{/if}

<style>
  .wave-legend {
    position: fixed;
    right: 8px;
    bottom: 8px;
    z-index: 9999;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 6px 8px;
    border: 1px solid var(--ff-border-hi);
    border-radius: 6px;
    background: var(--ff-surface);
    color: var(--ff-text-mid);
    font: 11px/1.4 ui-monospace, monospace;
  }
  .wave-legend__title {
    color: var(--ff-text);
    font-weight: 600;
  }
  .wave-legend__row {
    all: unset;
    cursor: pointer;
    display: flex;
    gap: 6px;
    align-items: baseline;
  }
  .wave-legend__row--on {
    color: var(--ff-text);
  }
  kbd {
    padding: 0 4px;
    border: 1px solid var(--ff-border-hi);
    border-radius: 3px;
    font: inherit;
  }
</style>
