<script>
  import { Button, ContextMenu, Icon } from '$ds';
  import { nativeListener } from '$ds/components/nativeListener.js';
  import {
    ui,
    expandOrganize,
    collapseOrganize,
    clearOrganizeTarget,
    setOrganizeWidth,
    commitOrganizeWidth,
    ORGANIZE_SNAP_TO_RAIL_PX,
    ORGANIZE_SNAP_TO_FULL_PX,
  } from '../stores/ui.svelte.ts';
  import { dnd } from '../stores/dnd.svelte.ts';
  import { dropFlash, cardKey, boardDropTarget } from '../stores/organize-drop.svelte.ts';
  import { renamePlaylist, tracksForSidebarSource } from '../stores/library.svelte.ts';
  import { artwork } from '../stores/artwork.svelte.ts';
  import { trackIdsWithCover } from '../lib/coverTracks.ts';
  import { buildSourceContextMenuItems } from '../menus/context-menus.ts';
  import { summarizeBoard, formatBoardDuration } from '../lib/boardSummary.ts';
  import { fitRibbonCovers } from '../lib/ribbonFit.ts';
  import { ORGANIZE_EMPTY_WIDTH, organizePanelWidth } from '../lib/panelWidth.ts';
  import OrganizePicker from './OrganizePicker.svelte';
  import TrackTable from './TrackTable.svelte';
  import LazyMount from '../lib/LazyMount.svelte';
  import { stickBoard } from '../lib/lazy-views.ts';

  let { containerEl = null } = $props();

  let renamingHeader = $state(false);
  let renameDraft = $state('');
  let renameHeaderEl = $state(null);
  let headerMenu = $state({ open: false, x: 0, y: 0 });
  let widthBeforeWiden = $state(null);

  function openHeaderMenu(e) {
    if (!ui.organize.target) return;
    e.preventDefault();
    headerMenu = { open: true, x: e.clientX, y: e.clientY };
  }

  function openTargetMenu(e) {
    const rect = e.currentTarget.getBoundingClientRect();
    headerMenu = { open: true, x: rect.left, y: rect.bottom + 2 };
  }

  function headerMenuItems() {
    const target = ui.organize.target;
    if (!target) return [];
    const sourceItems =
      target.kind === 'playlist'
        ? buildSourceContextMenuItems({
            sourceId: target.id,
            label: target.label,
            onRenamePlaylist: (playlist) => startHeaderRename(playlist.name),
          })
        : [];
    return sourceItems;
  }

  let drawerEl = $state(null);
  let pointerInDrawer = false;

  function onWindowPointerDown(e) {
    pointerInDrawer = !!drawerEl && drawerEl.contains(e.target);
  }

  function onWindowKeydown(e) {
    if (e.key !== 'Escape' || !drawerEl || !ui.organize.expanded || !ui.organize.target) return;
    if (renamingHeader || headerMenu.open) return;
    const focusInDrawer = drawerEl.contains(document.activeElement) || pointerInDrawer;
    if (!focusInDrawer) return;
    const el = e.target;
    if (el instanceof HTMLElement && (el.isContentEditable || ['INPUT', 'TEXTAREA'].includes(el.tagName))) return;
    e.preventDefault();
    e.stopPropagation();
    clearOrganizeTarget();
  }

  function startHeaderRename(name) {
    renamingHeader = true;
    renameDraft = name;
    setTimeout(() => renameHeaderEl?.select(), 0);
  }

  async function commitHeaderRename() {
    if (!renamingHeader) return;
    renamingHeader = false;
    const draft = renameDraft.trim();
    const target = ui.organize.target;
    if (!target || !draft || draft === target.label) return;
    if (target.kind === 'playlist' && typeof target.playlistId === 'number') {
      await renamePlaylist(target.playlistId, draft);
      ui.organize.target = { ...target, id: draft, label: draft };
    }
  }

  function cancelHeaderRename() {
    renamingHeader = false;
  }

  let railHeight = $state(0);

  const boardTracks = $derived(ui.organize.target ? tracksForSidebarSource(ui.organize.target.id) : []);

  const allRailCoverIds = $derived(trackIdsWithCover(boardTracks));

  const showingBoard = $derived(ui.organize.target !== null);
  const shownWidth = $derived(organizePanelWidth(ui.organize.width, showingBoard, ORGANIZE_EMPTY_WIDTH));

  const ribbonHover = $derived(dnd.kind === 'tracks' && dnd.dropRibbon);
  const boardDropKey = $derived.by(() => {
    const target = ui.organize.target && boardDropTarget(ui.organize.target);
    return target ? cardKey(target) : null;
  });
  const landed = $derived(!!boardDropKey && dropFlash.key === boardDropKey && dropFlash.added > 0);
  const slotMode = $derived(landed ? 'landed' : ribbonHover && ui.organize.target ? 'hint' : null);
  const railFit = $derived(fitRibbonCovers(allRailCoverIds, railHeight, slotMode ? 1 : 0));
  const railCovers = $derived(railFit.covers.map((id) => artwork.get(id, true)).filter(Boolean));

  $effect(() => artwork.keep('organize-rail', () => railFit.covers));
  const summary = $derived(summarizeBoard(boardTracks));
  const railTrackCount = $derived(summary.count);
  const subtitle = $derived(
    [
      `${summary.count} ${summary.count === 1 ? 'track' : 'tracks'}`,
      summary.durationSecs > 0 ? formatBoardDuration(summary.durationSecs) : null,
    ]
      .filter(Boolean)
      .join(' · ')
  );

  function fullWidth() {
    const right = containerEl ? containerEl.getBoundingClientRect().right : window.innerWidth;
    const left = containerEl ? containerEl.getBoundingClientRect().left : 0;
    return right - left - ui.sidebarWidth;
  }

  function toggleWiden() {
    if (widthBeforeWiden !== null) {
      setOrganizeWidth(widthBeforeWiden);
      widthBeforeWiden = null;
    } else {
      widthBeforeWiden = ui.organize.width;
      setOrganizeWidth(fullWidth());
    }
    commitOrganizeWidth();
  }

  function onResizeHandlePointerDown(e) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    widthBeforeWiden = null;
    e.currentTarget.setPointerCapture?.(e.pointerId);
    document.body.classList.add('ff-organize--resizing');

    function onMove(ev) {
      const containerRight = containerEl
        ? containerEl.getBoundingClientRect().right
        : window.innerWidth;
      applyResizeWidth(containerRight - ev.clientX, containerRight);
    }

    function onUp() {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      window.removeEventListener('pointercancel', onUp);
      document.body.classList.remove('ff-organize--resizing');
      commitOrganizeWidth();
    }

    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    window.addEventListener('pointercancel', onUp);
  }

  function applyResizeWidth(rawWidth, containerRight) {
    if (rawWidth < ORGANIZE_SNAP_TO_RAIL_PX) {
      collapseOrganize();
      return;
    }

    const containerLeft = containerEl ? containerEl.getBoundingClientRect().left : 0;
    const maxWidth = containerRight - containerLeft - ui.sidebarWidth;
    if (rawWidth > maxWidth - ORGANIZE_SNAP_TO_FULL_PX) {
      setOrganizeWidth(maxWidth);
      return;
    }

    setOrganizeWidth(rawWidth);
    if (!ui.organize.expanded) expandOrganize();
  }
</script>

<svelte:window onpointerdowncapture={onWindowPointerDown} onkeydowncapture={onWindowKeydown} />

{#if ui.organize.expanded}
  <div class="ff-organize ff-organize--expanded" style:width="{shownWidth}px" bind:this={drawerEl}>
    {#if showingBoard}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="ff-organize__resize-handle"
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize organize panel"
        {@attach nativeListener('pointerdown', onResizeHandlePointerDown)}
      ></div>
    {/if}

    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="ff-organize__header" oncontextmenu={openHeaderMenu}>
      {#if renamingHeader && ui.organize.target}
        <input
          bind:this={renameHeaderEl}
          class="ff-organize__header-rename"
          type="text"
          aria-label="Name"
          bind:value={renameDraft}
          onkeydown={(e) => {
            if (e.key === 'Enter') { e.preventDefault(); void commitHeaderRename(); }
            if (e.key === 'Escape') cancelHeaderRename();
          }}
          onblur={commitHeaderRename}
        />
      {:else if ui.organize.target}
        <Button
          icon="chevron-left"
          iconOnly
          variant="ghost"
          size="small"
          ariaLabel="Back to destinations"
          title="Back to destinations (Esc)"
          onclick={clearOrganizeTarget}
        />
        <button type="button" class="ff-organize__target" aria-label="Board menu" onclick={openTargetMenu}>
          <span class="ff-organize__target-name">{ui.organize.target.label}</span>
          {#if ui.organize.target.kind !== 'usb'}<span class="ff-organize__target-sub">{subtitle}</span>{/if}
          <Icon name="chevron-down" size={12} />
        </button>
      {:else}
        <span class="ff-organize__header-title">Put tracks into</span>
      {/if}
      <div class="ff-organize__header-actions">
        {#if showingBoard}
          <Button
            icon={widthBeforeWiden !== null ? 'minimize' : 'maximize'}
            iconOnly
            variant="ghost"
            size="small"
            ariaLabel={widthBeforeWiden !== null ? 'Narrow the board' : 'Widen to full window'}
            title={widthBeforeWiden !== null ? 'Narrow the board' : 'Widen to full window'}
            onclick={toggleWiden}
          />
        {/if}
        <Button
          icon="chevron-right"
          iconOnly
          variant="ghost"
          size="small"
          ariaLabel="Hide board"
          title="Hide board (⇧⇥)"
          onclick={collapseOrganize}
        />
      </div>
    </div>

    <div class="ff-organize__body" class:ff-organize__body--table={ui.organize.target !== null}>
      {#if ui.organize.target === null}
        <OrganizePicker />
      {:else if ui.organize.target.kind === 'usb'}
        <LazyMount view={stickBoard} props={{ volumePath: ui.organize.target.id }} />
      {:else}
        <TrackTable sourceId={ui.organize.target.id} embedded layoutKindOverride="organize" organize={true} />
      {/if}
    </div>

    {#if ui.organize.target && ui.organize.target.kind !== 'usb' && (summary.avgBpm !== null || summary.keyRange !== null)}
      <div class="ff-organize__foot">
        {#if summary.avgBpm !== null}<span>avg {Math.round(summary.avgBpm)} BPM</span>{/if}
        {#if summary.keyRange !== null}<span>{summary.keyRange}</span>{/if}
      </div>
    {/if}
  </div>
{:else}
  <button
    type="button"
    class="ff-organize ff-organize--rail{ui.organize.target ? ' ff-organize--rail-filled' : ' ff-organize--rail-empty'}"
    class:ff-organize--rail-drop={ribbonHover}
    data-drop-ribbon
    bind:clientHeight={railHeight}
    aria-label={ui.organize.target ? `Open sidebar: ${ui.organize.target.label} (${railTrackCount} tracks)` : 'Open organize sidebar'}
    aria-expanded="false"
    title="Open organize sidebar (⇧⇥)"
    onclick={expandOrganize}
  >
    <div class="ff-organize__rail-top" aria-hidden="true">
      <Icon name="chevron-left" size={18} />
    </div>
    {#if ui.organize.target}
      {#if slotMode}
        <div
          class="ff-organize__rail-tile ff-organize__rail-slot{landed ? ' ff-organize__rail-slot--landed' : ''}"
          data-rail-slot={slotMode}
          style:background-image={landed && dropFlash.cover ? `url(${dropFlash.cover})` : null}
          aria-hidden="true"
        >
          {#if landed}
            <span class="ff-organize__rail-count">+{dropFlash.added}</span>
          {:else}
            <Icon name="plus" size={16} />
          {/if}
        </div>
      {/if}
      {#each railCovers as coverUrl}
        <div class="ff-organize__rail-tile" style:background-image="url({coverUrl})" aria-hidden="true"></div>
      {/each}
      {#if railFit.more > 0}
        <div class="ff-organize__rail-tile ff-organize__rail-more" data-rail-more aria-hidden="true">+{railFit.more}</div>
      {/if}
    {/if}
  </button>
{/if}

{#if headerMenu.open}
  <ContextMenu
    open
    x={headerMenu.x}
    y={headerMenu.y}
    items={headerMenuItems()}
    onClose={() => (headerMenu = { ...headerMenu, open: false })}
  />
{/if}

<style>
  .ff-organize {
    position: relative;
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
    margin: var(--ff-space-3, 8px) 0;
    background: var(--ff-surface);
    border: 1px solid var(--ff-border);
    border-right: none;
    border-radius: 10px 0 0 10px;
    overflow: hidden;
  }

  .ff-organize--rail {
    cursor: pointer;
    align-items: center;
    touch-action: none;
    user-select: none;
    color: var(--ff-text-mid);
    padding: 0;
    font: inherit;
    transition: background 0.12s ease, border-color 0.12s ease, color 0.12s ease;
  }

  .ff-organize--rail:focus-visible {
    outline: 1px solid var(--ff-accent);
    outline-offset: -2px;
  }

  .ff-organize--rail:active {
    background: var(--ff-select);
  }

  .ff-organize--rail:hover {
    background: var(--ff-elev);
    border-color: var(--ff-border-hi);
    color: var(--ff-text);
  }

  .ff-organize--rail {
    overflow: visible;
  }

  .ff-organize--rail-drop,
  .ff-organize--rail-drop:hover {
    box-shadow: inset 2px 0 0 var(--ff-accent);
  }

  .ff-organize__rail-slot {
    display: flex;
    align-items: center;
    justify-content: center;
    box-sizing: border-box;
    border: 1px dashed var(--ff-border-hi);
    background-color: transparent;
    color: var(--ff-text-mid);
    animation: ff-rail-slot-in 0.14s ease-out;
  }
  .ff-organize__rail-slot--landed {
    border-style: solid;
    border-color: transparent;
    background-color: var(--ff-elev);
    background-size: cover;
    background-position: center;
    animation: ff-rail-land 0.22s ease-out;
  }
  .ff-organize__rail-count {
    padding: 1px 5px;
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.55);
    color: var(--ff-text);
    font-size: var(--ff-type-small);
    font-variant-numeric: tabular-nums;
  }
  .ff-organize__rail-more {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--ff-text-mid);
    font-size: var(--ff-type-small);
    font-variant-numeric: tabular-nums;
  }
  @keyframes ff-rail-slot-in {
    from { opacity: 0; transform: translateY(-10px); }
    to { opacity: 1; transform: none; }
  }
  @keyframes ff-rail-land {
    from { opacity: 0.4; transform: translateY(-14px) scale(0.85); }
    to { opacity: 1; transform: none; }
  }

  .ff-organize--rail-empty {
    width: 44px;
    justify-content: center;
  }
  .ff-organize--rail-empty .ff-organize__rail-top {
    width: 100%;
    height: 100%;
  }

  .ff-organize--rail-filled {
    width: 52px;
    justify-content: flex-start;
    gap: 6px;
    padding: 6px;
  }
  .ff-organize--rail-filled.ff-organize--rail { padding: 6px; }

  .ff-organize__rail-top {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 28px;
    flex-shrink: 0;
  }

  .ff-organize__rail-tile {
    width: 40px;
    height: 40px;
    border-radius: 8px;
    background-color: var(--ff-elev);
    background-size: cover;
    background-position: center;
    flex-shrink: 0;
  }

  .ff-organize--expanded {
    min-width: 0;
  }

  .ff-organize__resize-handle {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 5px;
    cursor: col-resize;
    touch-action: none;
    z-index: 4;
  }

  .ff-organize__resize-handle::after {
    content: '';
    position: absolute;
    left: 2px;
    top: 0;
    bottom: 0;
    width: 1px;
    background: var(--ff-border-hi);
    opacity: 0;
    transition: opacity 0.12s ease, background 0.12s ease;
  }

  .ff-organize__resize-handle:hover::after {
    opacity: 1;
    background: var(--ff-border-hover, var(--ff-border-hi));
  }

  :global(body.ff-organize--resizing) .ff-organize__resize-handle::after {
    opacity: 1;
    background: var(--ff-accent);
  }

  :global(body.ff-organize--resizing) {
    cursor: col-resize !important;
    user-select: none;
  }

  :global(body.ff-organize--resizing) * {
    cursor: col-resize !important;
  }

  .ff-organize__header {
    display: flex;
    align-items: center;
    flex-shrink: 0;
    height: 40px;
    padding: 0 var(--ff-space-4) 0 var(--ff-space-5);
    gap: var(--ff-space-2);
    border-bottom: 1px solid var(--ff-border);
  }

  .ff-organize__header-title {
    flex: 1 1 0;
    font-size: var(--ff-type-body);
    font-weight: 500;
    color: var(--ff-text);
  }

  .ff-organize__target {
    flex: 1 1 0;
    min-width: 0;
    display: flex;
    align-items: baseline;
    gap: var(--ff-space-4);
    padding: var(--ff-space-2) var(--ff-space-3);
    border: none;
    border-radius: var(--ff-radius-sm);
    background: none;
    color: var(--ff-text);
    font-family: var(--ff-font);
    text-align: left;
    cursor: pointer;
  }

  .ff-organize__target:hover {
    background: var(--ff-hover);
  }

  .ff-organize__target-name {
    font-size: var(--ff-type-body);
    font-weight: 500;
    white-space: nowrap;
  }

  .ff-organize__target-sub {
    flex: 1;
    min-width: 0;
    font-size: var(--ff-type-small);
    color: var(--ff-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ff-organize__header-rename {
    flex: 1 1 0;
    background: var(--ff-elev);
    border: 1px solid var(--ff-accent);
    border-radius: var(--ff-radius-sm, 4px);
    color: var(--ff-text);
    font-size: var(--ff-type-body);
    font-weight: 500;
    padding: 2px var(--ff-space-2);
    outline: none;
    min-width: 0;
  }

  .ff-organize__header-actions {
    display: flex;
    align-items: center;
    gap: var(--ff-space-1);
    flex-shrink: 0;
  }

  .ff-organize__body {
    flex: 1 1 0;
    min-height: 0;
    display: flex;
    overflow: hidden;
  }

  .ff-organize__body--table {
    align-items: stretch;
    justify-content: stretch;
  }

  .ff-organize__body > :global(*) {
    flex: 1 1 0;
    min-width: 0;
    min-height: 0;
  }

  .ff-organize__foot {
    flex: none;
    display: flex;
    gap: var(--ff-space-6);
    padding: var(--ff-space-4) var(--ff-space-6);
    border-top: 1px solid var(--ff-border);
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-small);
    color: var(--ff-muted);
  }
</style>
