<script>
  import { Icon } from '$ds';
  import { ui, openPalette } from '../stores/ui.svelte.ts';

  let active = $derived(ui.paletteOpen || ui.searchText.trim() !== '');
</script>

<button
  type="button"
  class="ff-search-icon"
  class:ff-search-icon--active={active}
  title="Search (⌘F)"
  aria-label="Search"
  data-search-icon
  onclick={openPalette}
>
  <Icon name="search" size={14} />
</button>

<style>
  .ff-search-icon {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--ff-titlebar-control-height);
    height: var(--ff-titlebar-control-height);
    padding: 0;
    border: 1px solid transparent;
    border-radius: var(--ff-radius-md);
    background: none;
    color: var(--ff-text-mid);
    cursor: pointer;
    transition:
      color var(--ff-motion-fast) var(--ff-easing),
      background-color var(--ff-motion-fast) var(--ff-easing);
  }
  .ff-search-icon:hover,
  .ff-search-icon--active {
    background: var(--ff-hover);
    color: var(--ff-text);
  }
  .ff-search-icon:focus-visible {
    outline: none;
    border-color: var(--ff-border-hi);
  }
</style>
