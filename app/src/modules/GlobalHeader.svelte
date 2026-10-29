<!--
  GlobalHeader.svelte — Top header bar with search field and actions.
  The search field is the command palette's input: focusing it, ⌘F, ⌘K or ⌥↩ opens the dropdown
  under it (CommandPalette.svelte). Plain text still narrows the active track list live;
  a leading `>` switches the dropdown to commands only and leaves the list alone.
  Layout follows main-window option A: context title, search, status chips.
  The centre column width picks the search form: field (>= 720 px), an icon in this bar (>= 440 px),
  or an icon in the left sidebar's top-right corner. Either icon opens the palette as a centred modal over the window.
  While the organize sidebar is open the chips are dropped (the status bar
  already carries analysis and sync progress) so the narrower bar stays calm.
-->
<script>
  import { ContextMenu, FilterInput, Spinner, StatusDot } from '$ds';
  import { library, sidebarSourceLabel, tracksForSidebarSource, filterTracks } from '../stores/library.svelte.ts';
  import { ui, clearListFilter, setSearchText, openPalette, dockOrganizeTarget } from '../stores/ui.svelte.ts';
  import { stickSync } from '../stores/stick-sync.svelte.ts';
  import { stickAllTrackIds } from '../stores/sticks.svelte.ts';
  import { searchModeForColumn } from '../lib/searchLayout.ts';
  import SearchIconButton from './SearchIconButton.svelte';
  import { stickLabel, topbarIdleStatus } from '../lib/topbarStatus.ts';
  import { fileStatus } from '../stores/file-status.svelte.ts';
  import { checkLibraryAgain, openHealthDialog, relocateMissing, showProblemTracks } from '../stores/file-health.svelte.ts';
  import { problemLabel } from '../lib/fileHealth.ts';

  let filterInputEl = $state(null);
  let statusMenu = $state({ open: false, x: 0, y: 0 });
  let problems = $derived(problemLabel(fileStatus));
  let statusMenuItems = $derived([
    { label: 'Relocate missing…', disabled: fileStatus.missing === 0 || fileStatus.relocating, onClick: () => void relocateMissing() },
    fileStatus.onlyProblems
      ? { label: 'Show all tracks', onClick: () => showProblemTracks(false) }
      : { label: 'Show missing tracks', onClick: () => showProblemTracks(true) },
    { label: 'Check again', disabled: fileStatus.running, onClick: () => void checkLibraryAgain() },
    { isSeparator: true },
    { label: 'Library check results…', onClick: openHealthDialog },
  ]);
  let searchEl = $state(null);
  let headerEl = $state(null);
  let returnFocusEl = null;

  let activeTracks = $derived(tracksForSidebarSource(ui.activeSidebarRow));
  let filteredCount = $derived(filterTracks(activeTracks, ui.listFilter).length);
  let showingProblems = $derived(fileStatus.onlyProblems && ui.activeSidebarRow === 'All Tracks');
  let contextLabel = $derived(showingProblems ? 'Missing tracks' : sidebarSourceLabel(ui.activeSidebarRow));
  let searchCollapsed = $derived(ui.searchMode !== 'field');
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
    if (!headerEl) return;
    const observer = new ResizeObserver(([entry]) => {
      ui.searchMode = searchModeForColumn(entry.borderBoxSize[0].inlineSize);
    });
    observer.observe(headerEl);
    return () => {
      observer.disconnect();
      ui.searchMode = 'field';
    };
  });

  $effect(() => {
    ui.searchAnchor = searchEl;
    return () => {
      ui.searchAnchor = null;
    };
  });

  $effect(() => {
    if (!filterInputEl) return;
    if (ui.paletteOpen && !searchCollapsed) {
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

  let statusButtonEl = $state(null);
  let closedUnderPointerAt = -Infinity;

  function closeStatusMenu() {
    statusMenu = { ...statusMenu, open: false };
    if (statusButtonEl?.matches(':hover')) closedUnderPointerAt = performance.now();
  }

  function toggleStatusMenu(e) {
    if (statusMenu.open || performance.now() - closedUnderPointerAt < 400) {
      closedUnderPointerAt = -Infinity;
      statusMenu = { ...statusMenu, open: false };
      return;
    }
    const rect = e.currentTarget.getBoundingClientRect();
    statusMenu = { open: true, x: rect.left, y: rect.bottom + 4 };
  }

  function openIdleStick(volume) {
    dockOrganizeTarget({ kind: 'usb', id: volume, label: stickLabel(volume) });
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

<div class="ff-gheader ff-titlebar-band ff-titlebar-band--ruled" class:ff-gheader--icon={ui.searchMode === 'icon'} data-tauri-drag-region bind:this={headerEl}>
  <div class="ff-gheader__ctx" data-tauri-drag-region>
    <b class="ff-gheader__title">{contextLabel}</b>
    <span class="ff-gheader__count">{(showingProblems ? fileStatus.missing + fileStatus.error : activeTracks.length).toLocaleString()}</span>
  </div>
  <div class="ff-gheader__searchslot" class:ff-gheader__searchslot--cover={searchCollapsed}>
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
  </div>
  <div class="ff-gheader__right" data-tauri-drag-region>
    {#if problems}
      <button
        type="button"
        class="ff-gheader__chip ff-gheader__chip--idle ff-gheader__chip--warn"
        data-library-status
        bind:this={statusButtonEl}
        aria-haspopup="menu"
        aria-expanded={statusMenu.open}
        onclick={toggleStatusMenu}
      >
        <StatusDot status="warn" /> {problems}
      </button>
    {/if}
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
        {#if idleStatus.kind === 'stick'}
          <button type="button" class="ff-gheader__chip ff-gheader__chip--idle" onclick={() => openIdleStick(idleStatus.volume)}>
            <StatusDot status="online" /> {idleStatus.name} <b>{idleStatus.detail}</b>
          </button>
        {:else}
          <span class="ff-gheader__plain">{idleStatus.text}</span>
        {/if}
      {/if}
    {/if}
  </div>
  {#if ui.searchMode === 'icon'}
    <SearchIconButton />
  {/if}
</div>

<ContextMenu
  open={statusMenu.open && !!problems}
  x={statusMenu.x}
  y={statusMenu.y}
  items={statusMenuItems}
  onClose={closeStatusMenu}
/>

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
  .ff-gheader--icon {
    padding-right: var(--ff-space-4);
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
  .ff-gheader__searchslot {
    display: flex;
    align-items: center;
    flex: 1 1 0;
    min-width: 240px;
    max-width: 560px;
  }
  .ff-gheader__searchslot--cover {
    position: absolute;
    inset: 0;
    z-index: 2;
    flex: none;
    max-width: none;
    min-width: 0;
    padding: 0 var(--ff-space-7);
    background: var(--ff-bg);
    visibility: hidden;
    pointer-events: none;
  }
  .ff-gheader__search {
    display: flex;
    align-items: center;
    flex: 1 1 0;
    max-width: 560px;
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
    flex: 0 1 auto;
    margin-left: auto;
    min-width: 0;
    overflow: hidden;
  }
  .ff-gheader__plain {
    font-size: 11.5px;
    color: var(--ff-muted);
    white-space: nowrap;
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
  .ff-gheader__chip--warn {
    color: var(--ff-status-warn);
  }
  .ff-gheader__chip b {
    font-weight: 500;
    color: var(--ff-text);
    font-family: var(--ff-font-mono);
    font-size: 11px;
  }
</style>
