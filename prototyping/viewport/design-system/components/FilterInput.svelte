<!--
  FilterInput.svelte — Inline list filter (design-system §2.12).
-->
<script>
  import Icon from './Icon.svelte';
  import './filter-input.css';

  let {
    value = $bindable(''),
    placeholder = 'Filter…',
    count = '',
    onClose,
    onkeydown,
    inputRef = $bindable(null),
  } = $props();
</script>

<div class="ff-filter-input">
  <Icon name="search" size={12} class="ff-filter-input__search-icon" />
  <input
    bind:this={inputRef}
    class="ff-filter-input__field"
    type="search"
    {placeholder}
    bind:value
    {onkeydown}
    autocomplete="off"
    spellcheck="false"
  />
  {#if count}
    <span class="ff-filter-input__count">{count}</span>
  {/if}
  {#if value.trim()}
    <button
      type="button"
      class="ff-filter-input__close-btn"
      aria-label="Clear filter"
      onclick={() => {
        value = '';
        onClose?.();
      }}
    >
      ×
    </button>
  {/if}
</div>
