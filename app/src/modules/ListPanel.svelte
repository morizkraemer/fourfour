<!--
  ListPanel.svelte — Track list column with panel header (split / curate layout).
-->
<script>
  import { Button, ContextMenu } from '$ds';
  import TrackTable from './TrackTable.svelte';
  import { tracksForSidebarSource, filterTracks } from '../stores/library.svelte.ts';
  import { focusListPanel, ui, openSplitPanel } from '../stores/ui.svelte.ts';
  import { buildSourceContextMenuItems } from '../menus/context-menus.ts';
  import '../../../prototyping/viewport/design-system/components/panel-header.css';

  let {
    sourceId,
    title,
    showClose = false,
    splitCompanion = false,
    onClose,
  } = $props();

  let contextMenu = $state({ open: false, x: 0, y: 0 });

  let tracks = $derived(tracksForSidebarSource(sourceId));
  let visibleCount = $derived(filterTracks(tracks, ui.listFilter).length);
  let subtitle = $derived.by(() => {
    const total = tracks.length;
    if (ui.listFilter.trim()) {
      return `${visibleCount.toLocaleString()} of ${total.toLocaleString()} track${total === 1 ? '' : 's'}`;
    }
    return `${total.toLocaleString()} track${total === 1 ? '' : 's'}`;
  });

  function openHeaderMenu(e) {
    e.preventDefault();
    focusListPanel(sourceId);
    contextMenu = { open: true, x: e.clientX, y: e.clientY };
  }

  function headerMenuItems() {
    return buildSourceContextMenuItems({
      sourceId,
      label: title,
      onSplit: () => openSplitPanel(sourceId, title),
    });
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="ff-list-panel" onmousedown={() => focusListPanel(sourceId)}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <header
    class="ff-panel-header ff-panel-header--library"
    oncontextmenu={openHeaderMenu}
  >
    <div class="ff-panel-header__left">
      <h2 class="ff-panel-header__title">{title}</h2>
      <span class="ff-panel-header__subtitle">{subtitle}</span>
    </div>
    {#if showClose}
      <div class="ff-panel-header__right">
        <Button
          icon="x"
          iconOnly
          variant="ghost"
          size="small"
          ariaLabel="Close side panel"
          class="ff-list-panel__close"
          onclick={onClose}
        />
      </div>
    {/if}
  </header>
  <TrackTable {sourceId} embedded {splitCompanion} />
</div>

<ContextMenu
  open={contextMenu.open}
  x={contextMenu.x}
  y={contextMenu.y}
  items={headerMenuItems()}
  onClose={() => (contextMenu = { ...contextMenu, open: false })}
/>

<style>
  .ff-list-panel {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    min-width: 0;
    background: var(--ff-bg);
  }
  .ff-list-panel :global(.ff-table) {
    flex: 1 1 0;
    min-height: 0;
  }
  .ff-list-panel :global(.ff-list-panel__close) {
    color: var(--ff-faint);
  }
  .ff-list-panel :global(.ff-list-panel__close:hover) {
    color: var(--ff-text);
  }
</style>
