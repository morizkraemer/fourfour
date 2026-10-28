<!--
  GlobalHeader.svelte — Top header bar with search field and actions.
  The search field is the command palette's input: focusing it, ⌘F, ⌘K or ⌥↩ opens the dropdown
  under it (CommandPalette.svelte). Plain text still narrows the active track list live;
  a leading `>` switches the dropdown to commands only and leaves the list alone.
  Layout follows main-window option A: context title, search, status chips.
  While the organize sidebar is open the chips are dropped (the status bar
  already carries analysis and sync progress) so the narrower bar stays calm.
-->
<script>
  import { FilterInput, Spinner, StatusDot } from '$ds';
  import { library, sidebarSourceLabel, tracksForSidebarSource, filterTracks } from '../stores/library.svelte.ts';
  import { ui, clearListFilter, setSearchText, openPalette, dockOrganizeTarget } from '../stores/ui.svelte.ts';
  import { stickSync } from '../stores/stick-sync.svelte.ts';
  import { stickAllTrackIds } from '../stores/sticks.svelte.ts';
  import { goToSource } from '../shortcuts/navigation.ts';
  import { stickLabel, topbarIdleStatus } from '../lib/topbarStatus.ts';

  let filterInputEl = $state(null);
  let searchEl = $state(null);
  let returnFocusEl = null;

  let activeTracks = $derived(tracksForSidebarSource(ui.activeSidebarRow));
  let filteredCount = $derived(filterTracks(activeTracks, ui.listFilter).length);
  let contextLabel = $derived(sidebarSourceLabel(ui.activeSidebarRow));
  let showChips = $derived(!ui.organize.expanded);
  let analyzing = $derived(library.analyzing && library.analysisProgress.total > 0);
  let idleStatus = $derived(
    topbarIdleStatus({
      volumes: library.volumes,
      activeVolume: library.activeVolume,
      stickTrackCount: (volume) => stickAllTrackIds(volume).length,
      trackCount: library.tracks.length,
      unanalyzedCount: library.tracks.filter((t) => !t.analyzed).length,
      sync: stickSync,
    }),
  );
  let filterCountLabel = $derived(
    ui.listFilter.trim()
      ? `${filteredCount} / ${activeTracks.length}`
      : ''
  );

  $effect(() => {
    ui.searchAnchor = searchEl;
    return () => {
      ui.searchAnchor = null;
    };
  });

  $effect(() => {
    if (!filterInputEl) return;
    if (ui.paletteOpen) {
      if (document.activeElement !== filterInputEl) {
        returnFocusEl = document.activeElement;
        filterInputEl.focus();
        filterInputEl.select();
      }
    } else if (document.activeElement === filterInputEl) {
      if (returnFocusEl?.isConnected && returnFocusEl !== document.body) returnFocusEl.focus();
      else filterInputEl.blur();
      returnFocusEl = null;
    }
  });

  function openIdleStatus() {
    if (idleStatus.kind === 'stick') {
      dockOrganizeTarget({ kind: 'usb', id: idleStatus.volume, label: stickLabel(idleStatus.volume) });
    } else {
      void goToSource('All Tracks');
    }
  }

  function onSearchFocusIn(e) {
    if (e.target !== filterInputEl || ui.paletteOpen) return;
    returnFocusEl = e.relatedTarget;
    openPalette();
  }

  function onSearchClick() {
    if (!ui.paletteOpen) openPalette();
  }

  function onSearchInput() {
    if (!ui.paletteOpen) openPalette();
  }

  function onFilterKeydown(e) {
    if (e.key === 'Escape' && !ui.paletteOpen) {
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
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div class="ff-gheader__search" class:ff-gheader__search--open={ui.paletteOpen} bind:this={searchEl} onfocusin={onSearchFocusIn} oninput={onSearchInput} onclick={onSearchClick}>
    <FilterInput
      bind:value={() => ui.searchText, setSearchText}
      bind:inputRef={filterInputEl}
      placeholder="Search tracks and commands"
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
      {#if library.syncing}
        <span class="ff-gheader__chip"><Spinner size="small" /> Syncing</span>
      {:else if !analyzing}
        <button type="button" class="ff-gheader__chip ff-gheader__chip--idle" onclick={openIdleStatus}>
          {#if idleStatus.kind === 'stick'}
            <StatusDot status="online" /> {idleStatus.name} <b>{idleStatus.detail}</b>
          {:else}
            {idleStatus.text}
          {/if}
        </button>
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
  .ff-gheader__search {
    transition:
      max-width var(--ff-motion-fast) var(--ff-easing),
      flex-grow var(--ff-motion-fast) var(--ff-easing);
  }
  .ff-gheader__search--open {
    flex-grow: 3;
    max-width: 560px;
  }
  .ff-gheader__search :global(.ff-filter-input) {
    flex: 1 1 0;
    min-width: 0;
    height: var(--ff-titlebar-control-height);
    border-radius: var(--ff-radius-md);
    background: var(--ff-surface);
    border-color: var(--ff-border);
  }
  .ff-gheader__search--open :global(.ff-filter-input) {
    background: var(--ff-elev);
    border-color: var(--ff-border-hi);
    border-bottom-color: var(--ff-border);
    border-radius: var(--ff-radius-md) var(--ff-radius-md) 0 0;
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
  .ff-gheader__chip--idle {
    background: none;
    font-family: inherit;
    color: var(--ff-text-mid);
    cursor: pointer;
    transition:
      color var(--ff-motion-fast) var(--ff-easing),
      background-color var(--ff-motion-fast) var(--ff-easing);
  }
  .ff-gheader__chip--idle:hover {
    background: var(--ff-hover);
    color: var(--ff-text);
  }
  .ff-gheader__chip b {
    font-weight: 500;
    color: var(--ff-text);
    font-family: var(--ff-font-mono);
    font-size: 11px;
  }
</style>
