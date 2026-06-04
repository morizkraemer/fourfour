<script>
  import './segmented-control.css';

  let {
    items = [],
    activeIndex = 0,
    disabled = false,
    onChange,
  } = $props();

  /** @param {number} index */
  function select(index) {
    if (disabled || index === activeIndex) return;
    onChange?.(index);
  }
</script>

<div class="ff-segmented" class:ff-segmented--disabled={disabled}>
  {#each items as label, i (label)}
    <button
      type="button"
      class="ff-segmented__item"
      class:ff-segmented__item--on={i === activeIndex}
      {disabled}
      onclick={() => select(i)}
    >
      {label}
    </button>
  {/each}
</div>

<style>
  .ff-segmented--disabled {
    opacity: 0.55;
    pointer-events: none;
  }
</style>
