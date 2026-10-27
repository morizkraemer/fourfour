<!--
  GlobalHeader.svelte — Top header bar with search field and actions.
  Search is the ⌘F inline filter (narrows the active track list in place).
  Layout follows main-window option A: context title, search, status chips.
  While the organize sidebar is open the chips are dropped (the status bar
  already carries analysis and sync progress) so the narrower bar stays calm.
-->
<script>
  import { FilterInput, Spinner, StatusDot } from '$ds';
  import { library, sidebarSourceLabel, tracksForSidebarSource, filterTracks } from '../stores/library.svelte.ts';
  import { ui, clearListFilter } from '../stores/ui.svelte.ts';

  let filterInputEl = $state(null);

  let activeTracks = $derived(tracksForSidebarSource(ui.activeSidebarRow));
  let filteredCount = $derived(filterTracks(activeTracks, ui.listFilter).length);
  let contextLabel = $derived(sidebarSourceLabel(ui.activeSidebarRow));
  let showChips = $derived(!ui.organize.expanded);
  let analyzing = $derived(library.analyzing && library.analysisProgress.total > 0);
  let volumeName = $derived(library.activeVolume ? sidebarSourceLabel(library.activeVolume) : '');
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
  <div class="ff-gheader__ctx" data-tauri-drag-region>
    <b class="ff-gheader__title">{contextLabel}</b>
    <span class="ff-gheader__count">{activeTracks.length.toLocaleString()}</span>
  </div>
  <div class="ff-gheader__search">
    <FilterInput
      bind:value={ui.listFilter}
      bind:inputRef={filterInputEl}
      placeholder="Search tracks, artists, labels…"
      count={filterCountLabel}
      onClose={clearListFilter}
      onkeydown={onFilterKeydown}
    />
  </div>
  <div class="ff-gheader__right" data-tauri-drag-region>
    {#if showChips}
      {#if analyzing}
        <span class="ff-gheader__chip">
          <Spinner size="small" /> Analyzing
          <b>{library.analysisProgress.current} / {library.analysisProgress.total}</b>
        </span>
      {/if}
      {#if volumeName}
        <span class="ff-gheader__chip">
          <StatusDot status="online" /> {volumeName}
          {#if library.syncing}<b>Syncing</b>{/if}
        </span>
      {/if}
    {/if}
  </div>
</div>

<style>
  .ff-gheader {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--ff-space-6);
    width: 100%;
    padding-left: var(--ff-space-7);
    padding-right: var(--ff-space-7);
    overflow: hidden;
    background: var(--ff-bg);
  }
  .ff-gheader__ctx {
    display: flex;
    align-items: baseline;
    gap: var(--ff-space-4);
    flex: 0 1 170px;
    min-width: 0;
  }
  .ff-gheader__title {
    font-size: 14px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ff-gheader__count {
    font-family: var(--ff-font-mono);
    font-size: 11px;
    color: var(--ff-muted);
  }
  .ff-gheader__search {
    display: flex;
    align-items: center;
    flex: 1 1 0;
    max-width: 420px;
    min-width: 0;
  }
  .ff-gheader__search :global(.ff-filter-input) {
    flex: 1 1 0;
    min-width: 0;
    height: var(--ff-titlebar-control-height);
    border-radius: var(--ff-radius-md);
    background: var(--ff-surface);
    border-color: var(--ff-border);
  }
  .ff-gheader__right {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--ff-space-4);
    flex: 1 1 0;
    min-width: 0;
    overflow: hidden;
  }
  .ff-gheader__chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 24px;
    padding: 0 9px;
    border: 1px solid var(--ff-border);
    border-radius: var(--ff-radius-md);
    font-size: 11.5px;
    color: var(--ff-text-mid);
    white-space: nowrap;
    flex: none;
  }
  .ff-gheader__chip b {
    font-weight: 500;
    color: var(--ff-text);
    font-family: var(--ff-font-mono);
    font-size: 11px;
  }
</style>
