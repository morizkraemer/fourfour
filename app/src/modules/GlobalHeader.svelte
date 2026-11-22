<!--
  GlobalHeader.svelte — Top header bar with search field and actions.
  The search field is the command palette's input: focusing it, ⌘F, ⌘K or ⌥↩ opens the dropdown
  under it (CommandPalette.svelte). Plain text still narrows the active track list live;
  a leading `>` switches the dropdown to commands only and leaves the list alone.
  Layout follows main-window option A: context title, search, status chips.
  The centre column width picks the search form: field (>= 720 px), an icon in this bar (>= 440 px),
  or an icon in the left sidebar's top-right corner. Either icon opens the palette as a centred modal over the window.
-->
<script>
  import { FilterInput, Spinner, StatusDot } from '$ds';
  import StickDot from './StickDot.svelte';
  import { library, sidebarSourceLabel, tracksForSidebarSource, filterTracks } from '../stores/library.svelte.ts';
  import { ui, clearListFilter, setSearchText, openPalette } from '../stores/ui.svelte.ts';
  import { statusOfStick } from '../stores/stick-sync.svelte.ts';
  import { showStickInfo } from '../shortcuts/navigation.ts';
  import { searchModeForColumn } from '../lib/searchLayout.ts';
  import SearchIconButton from './SearchIconButton.svelte';
  import LazyMount from '../lib/LazyMount.svelte';
  import { libraryActivityMenu } from '../lib/lazy-views.ts';
  import { topbarStick } from '../lib/stickStatus.ts';
  import { fileStatus } from '../stores/file-status.svelte.ts';
  import { problemLabel } from '../lib/fileHealth.ts';
  import { libraryActivity } from '../lib/libraryActivity.ts';
  import { importActivity } from '../stores/import-activity.svelte.ts';

  let filterInputEl = $state(null);
  let statusMenu = $state({ open: false, x: 0, y: 0 });
  let problems = $derived(problemLabel(fileStatus));
  let activity = $derived(
    libraryActivity({
      importing: importActivity.running,
      importDone: importActivity.done,
      importTotal: importActivity.total,
      syncing: library.syncing,
      analyzing,
      analysisCurrent: library.analysisProgress.current,
      analysisTotal: library.analysisProgress.total,
      resultHeadline: importActivity.resultVisible ? (importActivity.summary?.chip ?? null) : null,
      resultFailed: importActivity.failed,
      problems,
      trackCount: library.tracks.length,
    }),
  );
  let searchEl = $state(null);
  let headerEl = $state(null);
  let returnFocusEl = null;

  let activeTracks = $derived(tracksForSidebarSource(ui.activeSidebarRow));
  let filteredCount = $derived(filterTracks(activeTracks, ui.listFilter).length);
  let showingProblems = $derived(fileStatus.onlyProblems && ui.activeSidebarRow === 'All Tracks');
  let contextLabel = $derived(showingProblems ? 'Missing tracks' : sidebarSourceLabel(ui.activeSidebarRow));
  let searchCollapsed = $derived(ui.searchMode !== 'field');
  let analyzing = $derived(library.analyzing && library.analysisProgress.total > 0);
  let stick = $derived(topbarStick(library.volumes.map((volume) => ({ volume, status: statusOfStick(volume) }))));
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
    <button
      type="button"
      class="ff-gheader__chip ff-gheader__chip--idle ff-gheader__activity"
      class:ff-gheader__chip--warn={activity.kind === 'problems' || (activity.kind === 'result' && activity.failed)}
      data-library-status
      data-activity={activity.kind}
      title="Library activity"
      bind:this={statusButtonEl}
      aria-haspopup="menu"
      aria-expanded={statusMenu.open}
      onclick={toggleStatusMenu}
    >
      {#if activity.kind === 'importing'}
        <Spinner size="small" /> <span class="ff-gheader__activity-label">{activity.label}</span>
        <span class="ff-gheader__progress" data-activity-progress class:ff-gheader__progress--indeterminate={activity.fraction === null} style:width={activity.fraction === null ? undefined : `${activity.fraction * 100}%`}></span>
      {:else if activity.kind === 'syncing'}
        <Spinner size="small" /> <span class="ff-gheader__activity-label">{activity.label}</span>
      {:else if activity.kind === 'analyzing'}
        <Spinner size="small" /> <span class="ff-gheader__activity-label">{activity.label}</span> <b>{activity.detail}</b>
        <span class="ff-gheader__progress" data-activity-progress style:width={`${activity.fraction * 100}%`}></span>
      {:else if activity.kind === 'rest'}
        <span class="ff-gheader__activity-label">{activity.label}</span>
      {:else}
        <StatusDot status={activity.kind === 'result' && !activity.failed ? 'online' : 'warn'} />
        <span class="ff-gheader__activity-label">{activity.label}</span>
      {/if}
    </button>
    {#if stick}
      <button
        type="button"
        class="ff-gheader__chip ff-gheader__chip--idle"
        data-stick-chip={stick.status.kind}
        title="Show {stick.name}"
        onclick={() => void showStickInfo(stick.volume)}
      >
        <StickDot tone={stick.status.tone} label={stick.status.label} />
        {stick.name} <b>{stick.status.label}</b>{#if stick.others > 0}<span class="ff-gheader__chip-more">+{stick.others}</span>{/if}
      </button>
    {/if}
  </div>
  {#if ui.searchMode === 'icon'}
    <SearchIconButton />
  {/if}
</div>

<LazyMount
  view={libraryActivityMenu}
  needed={statusMenu.open}
  props={{ open: statusMenu.open, x: statusMenu.x, y: statusMenu.y, hasProblems: !!problems, onClose: closeStatusMenu }}
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
  .ff-gheader__activity {
    position: relative;
    overflow: hidden;
    max-width: 280px;
  }
  .ff-gheader__activity-label {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ff-gheader__progress {
    position: absolute;
    left: 0;
    bottom: 0;
    height: 2px;
    background: var(--ff-text-mid);
    transition: width var(--ff-motion-fast) var(--ff-easing);
  }
  .ff-gheader__progress--indeterminate {
    width: 30%;
    animation: ff-gheader-indeterminate 1.1s linear infinite;
  }
  @keyframes ff-gheader-indeterminate {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(340%);
    }
  }
  .ff-gheader__chip--warn {
    color: var(--ff-status-warn);
  }
  .ff-gheader__chip-more {
    color: var(--ff-muted);
  }
  .ff-gheader__chip b {
    font-weight: 500;
    color: var(--ff-text);
    font-family: var(--ff-font-mono);
    font-size: 11px;
  }
</style>
