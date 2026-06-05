<!--
  GlobalHeader.svelte — Top header bar with search field and actions.
  Search is the ⌘F inline filter (narrows the active track list in place).
-->
<script>
  import { Kbd, FilterInput } from '$ds';
  import { tracksForSidebarSource, filterTracks } from '../stores/library.svelte.ts';
  import { ui, clearListFilter } from '../stores/ui.svelte.ts';

  let filterInputEl = $state(null);

  let activeTracks = $derived(tracksForSidebarSource(ui.activeSidebarRow));
  let filteredCount = $derived(filterTracks(activeTracks, ui.listFilter).length);
  let filterCountLabel = $derived(
    ui.listFilter.trim()
      ? `${filteredCount} / ${activeTracks.length}`
      : ''
  );

  // Only steal focus on explicit ⌘F (nonce > 0), not on mount — otherwise
  // spacebar play/pause never reaches the window handler while search is focused.
  $effect(() => {
    if (ui.filterFocusNonce === 0) return;
    filterInputEl?.focus();
    filterInputEl?.select();
  });

  function onFilterKeydown(e) {
    if (e.key === 'Escape') {
      clearListFilter();
      filterInputEl?.blur();
      e.preventDefault();
    }
  }
</script>

<div class="ff-gheader ff-titlebar-band ff-titlebar-band--ruled" data-tauri-drag-region>
  <div class="ff-gheader__search">
    <FilterInput
      bind:value={ui.listFilter}
      bind:inputRef={filterInputEl}
      placeholder="Search tracks, artists, labels…"
      count={filterCountLabel}
      onClose={clearListFilter}
      onkeydown={onFilterKeydown}
    />
    <span class="ff-gheader__kbd"><Kbd key="⌘F" /></span>
  </div>
</div>

<style>
  .ff-gheader {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    padding-left: var(--ff-space-7);
    padding-right: var(--ff-space-7);
    overflow: hidden;
    background: var(--ff-bg);
  }
  .ff-gheader__search {
    display: flex;
    align-items: center;
    gap: var(--ff-space-3);
    width: min(420px, 100%);
    max-width: 100%;
  }
  .ff-gheader__search :global(.ff-filter-input) {
    flex: 1 1 0;
    min-width: 0;
    height: var(--ff-titlebar-control-height);
    border-radius: var(--ff-radius-md);
    background: var(--ff-surface);
    border-color: var(--ff-border);
  }
  .ff-gheader__kbd {
    flex-shrink: 0;
    opacity: 0.85;
  }
</style>
