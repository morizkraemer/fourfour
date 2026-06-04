<script>
  // Nudge (Svelte port of nudge.js). −/value/+ stepper used by the player.
  //   onChange receives 'decrement' | 'increment'
  //   variant: 'default' | 'compact' | 'large'
  import './nudge.css';

  let { value, label = '', onChange, variant = 'default' } = $props();

  function step(dir, e) {
    e.stopPropagation();
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
  <span class="ff-nudge__value">{value}</span>
  <button
    class="ff-nudge__btn ff-nudge__btn--inc"
    title={label ? `Increase ${label}` : 'Increase'}
    aria-label="Increase"
    onclick={(e) => step('increment', e)}>+</button>
</div>
