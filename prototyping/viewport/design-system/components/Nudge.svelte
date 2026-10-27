<script>
  // Nudge (Svelte port of nudge.js). −/value/+ stepper used by the player.
  //   onChange receives 'decrement' | 'increment'
  //   variant: 'default' | 'compact' | 'large'
  //   With `model` (bpmScrub / keyScrub) the value is a ScrubValue: − and + step it
  //   (Shift/Alt change the step size) and `onCommit` receives the new value.
  import './nudge.css';
  import ScrubValue from './ScrubValue.svelte';

  let { value, label = '', onChange, variant = 'default', model = undefined, onCommit = undefined, color = undefined } = $props();

  function step(dir, e) {
    e.stopPropagation();
    if (model && onCommit) {
      if (!model.valid(value)) return;
      const next = model.step(value, dir === 'increment' ? 1 : -1, model.tier(e));
      if (next !== value) onCommit(next);
      return;
    }
    onChange?.(dir);
  }

  let cls = $derived(
    [
      'ff-nudge',
      variant === 'compact' && 'ff-nudge--compact',
      variant === 'large' && 'ff-nudge--large',
    ]
      .filter(Boolean)
      .join(' ')
  );
</script>

<div class={cls} aria-label={label}>
  <button
    class="ff-nudge__btn ff-nudge__btn--dec"
    title={label ? `Decrease ${label}` : 'Decrease'}
    aria-label="Decrease"
    onclick={(e) => step('decrement', e)}>−</button>
  {#if model && onCommit}
    <span class="ff-nudge__value ff-nudge__value--scrub">
      <ScrubValue {model} {value} {label} {color} {onCommit} />
    </span>
  {:else}
    <span class="ff-nudge__value">{value}</span>
  {/if}
  <button
    class="ff-nudge__btn ff-nudge__btn--inc"
    title={label ? `Increase ${label}` : 'Increase'}
    aria-label="Increase"
    onclick={(e) => step('increment', e)}>+</button>
</div>
