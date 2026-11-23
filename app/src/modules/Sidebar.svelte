<!--
  Sidebar.svelte — Left navigation with playlist drop targets and row reorder.
-->
<script>
  import { onMount, onDestroy, untrack } from 'svelte';
  import { SidebarSection, SidebarRow, ContextMenu, Dialog, Button, ResizeHandle } from '$ds';
  import { getIsTauri } from '../services/tauri.svelte.ts';
  import {
    buildSourceContextMenuItems,
    buildSidebarSectionMenuItems,
    buildFolderContextMenuItems,
    buildPlaylistSelectionMenuItems,
  } from '../menus/context-menus.ts';
  import {
    addPlaylist,
    renamePlaylist,
    refreshVolumes,
    selectVolume,
    ejectVolume,
    library,
    movePlaylistsInTree,
    favoritePlaylistName,
    nextFavoriteSlot,
    canAddFavorite,
    removeFavoriteSlot,
    favoriteSlotFromName,
    canRemoveFavoriteSlot,
  } from '../stores/library.svelte.ts';
  import { ui, dockOrganizeTarget, setPanelWidth, commitPanelWidth, resetPanelWidth } from '../stores/ui.svelte.ts';
  import { PANEL_WIDTH } from '../lib/panelWidth.ts';
  import { createPlaylistAndOpen } from '../stores/new-playlist.svelte.ts';
  import { sidebarSourceLabel } from '../stores/library.svelte.ts';
  import { dnd, endDrag, stickTargetAt, updatePointer } from '../stores/dnd.svelte.ts';
  import { dropPlaylistsOnStick } from '../stores/organize-drop.svelte.ts';
  import { statusOfStick, toggleStickProblem } from '../stores/stick-sync.svelte.ts';
  import StickDot from './StickDot.svelte';
  import {
    sidebarSelection,
    selectSidebarPlaylist,
    clearSidebarSelection,
    pruneSidebarSelection,
    isMultiSelected,
  } from '../stores/sidebar-selection.svelte.ts';
  import { outermostIds, playlistIdsInSelection } from '../lib/playlistSelection.ts';
  import { toggleFolder } from '../stores/sidebar-folders.svelte.ts';
  import { resetActiveRowIfDeleted } from '../stores/sidebar-delete.ts';
  import SearchIconButton from './SearchIconButton.svelte';

  let intervalId;
  let renamingPlaylistId = $state(null);
  let renameDraft = $state('');
  let playlistDragIds = $state([]);
  /** @type {{ id: number, placement: 'before' | 'after' | 'into' } | null} */
  let playlistDrop = $state(null);
  let contextMenu = $state({
    open: false,
    x: 0,
    y: 0,
    kind: 'source',
    sourceId: '',
    label: '',
    section: null,
    folderId: 0,
    selectionIds: [],
  });

  onMount(() => {
    refreshVolumes();
    intervalId = setInterval(refreshVolumes, 5000);
  });

  onDestroy(() => {
    if (intervalId) clearInterval(intervalId);
  });

  const sidebarData = $derived(library.sidebarData);

  let playlistRowsRevealed = $state(false);
  let playlistRowsScheduled = false;

  $effect(() => {
    if (playlistRowsScheduled) return;
    if (!library.tracksLoaded && library.playlists.length === 0) return;
    playlistRowsScheduled = true;
    untrack(() => requestAnimationFrame(() => requestAnimationFrame(() => (playlistRowsRevealed = true))));
  });

  const shownPlaylistRows = $derived(playlistRowsRevealed ? sidebarData.playlists.rows : []);
  let playlistRowOrder = $derived(sidebarData.playlists.rows.map((row) => row.id));

  $effect(() => {
    pruneSidebarSelection(new Set(library.playlists.map((p) => p.id)));
  });

  function handleRowClick(id, isUsb = false, path = null) {
    clearSidebarSelection();
    ui.activeSidebarRow = id;
    if (isUsb) {
      if (ui.openInspectorOnSelect) ui.detailPaneOpen = true;
      selectVolume(path);
    } else {
      selectVolume(null);
    }
  }

  function handlePlaylistRowClick(e, row, isFolder) {
    const mods = { shift: e.shiftKey, toggle: e.metaKey || e.ctrlKey };
    if (mods.shift || mods.toggle) {
      selectSidebarPlaylist(playlistRowOrder, row.id, mods);
      return;
    }
    if (isFolder) toggleFolder(row.id);
    else handleRowClick(row.label);
    selectSidebarPlaylist(playlistRowOrder, row.id, mods);
  }

  async function handleAddPlaylist() {
    await createPlaylistAndOpen();
  }

  function startPlaylistRename(playlistId, currentName) {
    renamingPlaylistId = playlistId;
    renameDraft = currentName;
  }

  async function commitPlaylistRename(playlistId) {
    if (renamingPlaylistId !== playlistId) return;
    const pl = library.playlists.find((p) => p.id === playlistId);
    const draft = renameDraft.trim();
    renamingPlaylistId = null;
    if (!pl || !draft || draft === pl.name) return;
    const oldName = pl.name;
    await renamePlaylist(playlistId, draft);
    if (ui.activeSidebarRow === oldName) {
      ui.activeSidebarRow = draft;
    }
    const target = ui.organize.target;
    if (target?.playlistId === playlistId) {
      ui.organize.target = { ...target, id: draft, label: draft };
    }
  }

  $effect(() => {
    const request = ui.sidebarRenameRequest;
    if (!request) return;
    ui.sidebarRenameRequest = null;
    const playlist = library.playlists.find((p) => p.id === request.playlistId);
    if (playlist) startPlaylistRename(playlist.id, playlist.name);
  });

  function cancelPlaylistRename() {
    renamingPlaylistId = null;
  }

  function handleAddFavorite() {
    const slot = nextFavoriteSlot();
    if (slot == null) return;
    addPlaylist(favoritePlaylistName(slot));
  }

  let showAddFavorite = $derived(canAddFavorite());

  /** @type {{ id: number, slot: number, trackCount: number } | null} */
  let removeFavoriteDialog = $state(null);

  function requestRemoveFavorite(playlist) {
    const slot = favoriteSlotFromName(playlist.name);
    if (slot == null || !canRemoveFavoriteSlot(slot)) return;
    if (playlist.track_ids.length === 0) {
      void commitRemoveFavorite(playlist.id);
      return;
    }
    removeFavoriteDialog = {
      id: playlist.id,
      slot,
      trackCount: playlist.track_ids.length,
    };
  }

  async function commitRemoveFavorite(playlistId) {
    const removedName = await removeFavoriteSlot(playlistId);
    removeFavoriteDialog = null;
    if (!removedName) return;
    if (ui.activeSidebarRow === removedName) {
      ui.activeSidebarRow = 'All Tracks';
    }
  }

  let removeFavoriteDialogBody = $derived(
    removeFavoriteDialog
      ? `Favorite ${removeFavoriteDialog.slot} has ${removeFavoriteDialog.trackCount} track${
          removeFavoriteDialog.trackCount === 1 ? '' : 's'
        }. Removing it clears those assignments from this favorite slot.`
      : ''
  );

  function sidebarRowState(label, playlistId = null, isFolder = false) {
    if (playlistId != null && isDropTarget(playlistId)) return 'selected';
    if (playlistId != null && isMultiSelected(playlistId)) return 'selected';
    if (!isFolder && ui.activeSidebarRow === label) return 'active';
    if (playlistId != null && playlistDragIds.includes(playlistId)) return 'hover';
    return 'rest';
  }

  function isDropTarget(playlistId) {
    return dnd.dropPlaylistId === playlistId || dnd.dropFolderId === playlistId;
  }

  function rowClassExtra(playlistId) {
    if (playlistId != null && isDropTarget(playlistId)) return 'drag-over';
    if (playlistId != null && playlistDragIds.includes(playlistId)) return 'drag-source';
    if (playlistId != null && playlistDrop?.id === playlistId) return `drop-${playlistDrop.placement}`;
    return '';
  }

  function dropPlacementAt(rowEl, clientY, isFolder) {
    const rect = rowEl.getBoundingClientRect();
    const ratio = (clientY - rect.top) / rect.height;
    if (isFolder) return ratio < 0.25 ? 'before' : 'into';
    return ratio < 0.5 ? 'before' : 'after';
  }

  function playlistDropAt(x, y) {
    const hit = document.elementFromPoint(x, y);
    if (hit?.closest('[data-playlist-root-drop]')) return { id: 0, placement: 'into' };
    const rowEl = hit?.closest('[data-playlist-id]');
    if (!rowEl) return null;
    const id = Number(rowEl.getAttribute('data-playlist-id'));
    const isFolder = rowEl.hasAttribute('data-playlist-folder');
    return { id, placement: dropPlacementAt(rowEl, y, isFolder) };
  }

  async function applyPlaylistDrop(dragIds, drop) {
    if (drop.id === 0) return movePlaylistsInTree(dragIds, 0);
    const target = library.playlists.find((p) => p.id === drop.id);
    if (!target || dragIds.includes(target.id)) return false;
    if (drop.placement === 'into') return movePlaylistsInTree(dragIds, target.id);
    return movePlaylistsInTree(dragIds, target.parent_id, target.id, drop.placement === 'after');
  }

  function closeContextMenu() {
    contextMenu = { ...contextMenu, open: false, section: null };
  }

  function contextMenuItems() {
    if (contextMenu.kind === 'selection') {
      return buildPlaylistSelectionMenuItems({
        ids: contextMenu.selectionIds,
        onDeleted: resetActiveRowIfDeleted,
      });
    }
    if (contextMenu.kind === 'folder') {
      const folder = library.playlists.find((p) => p.id === contextMenu.folderId);
      return folder
        ? buildFolderContextMenuItems({ folder, onRename: (f) => startPlaylistRename(f.id, f.name) })
        : [];
    }
    if (contextMenu.kind === 'section' && contextMenu.section) {
      return buildSidebarSectionMenuItems(contextMenu.section, {
        onNewPlaylist: handleAddPlaylist,
        onAddFavorite: handleAddFavorite,
        canAddFavorite: showAddFavorite,
      });
    }
    return buildSourceContextMenuItems({
      sourceId: contextMenu.sourceId,
      label: contextMenu.label,
      onNewPlaylist: handleAddPlaylist,
      onAddFavorite: handleAddFavorite,
      onRemoveFavorite: requestRemoveFavorite,
      onRenamePlaylist: (playlist) => startPlaylistRename(playlist.id, playlist.name),
    });
  }

  function openSectionMenu(e, section) {
    e.preventDefault();
    contextMenu = {
      open: true,
      x: e.clientX,
      y: e.clientY,
      kind: 'section',
      sourceId: '',
      label: '',
      section,
    };
  }

  function openSelectionMenu(e) {
    e.preventDefault();
    contextMenu = {
      open: true,
      x: e.clientX,
      y: e.clientY,
      kind: 'selection',
      sourceId: '',
      label: '',
      section: null,
      folderId: 0,
      selectionIds: playlistRowOrder.filter((id) => sidebarSelection.ids.includes(id)),
    };
  }

  function openPlaylistRowMenu(e, row, isFolder) {
    if (isMultiSelected(row.id)) {
      openSelectionMenu(e);
      return;
    }
    selectSidebarPlaylist(playlistRowOrder, row.id, { shift: false, toggle: false });
    if (isFolder) openFolderMenu(e, row.id);
    else openSourceMenu(e, row.label, row.label);
  }

  function openFolderMenu(e, folderId) {
    e.preventDefault();
    contextMenu = {
      open: true,
      x: e.clientX,
      y: e.clientY,
      kind: 'folder',
      sourceId: '',
      label: '',
      section: null,
      folderId,
    };
  }

  function openSourceMenu(e, sourceId, label, isUsb = false, usbPath = null) {
    e.preventDefault();
    if (isUsb && usbPath) handleRowClick(usbPath, true, usbPath);
    else handleRowClick(sourceId);
    contextMenu = {
      open: true,
      x: e.clientX,
      y: e.clientY,
      kind: 'source',
      sourceId,
      label,
      section: null,
    };
  }

  function openMenuFromKeyboard(e) {
    const row = e.target instanceof Element ? e.target.closest('.ff-sbrow') : null;
    if (!row || e.target !== row) return;
    e.preventDefault();
    const rect = row.getBoundingClientRect();
    row.dispatchEvent(
      new MouseEvent('contextmenu', {
        bubbles: true,
        cancelable: true,
        clientX: rect.left + 16,
        clientY: rect.bottom - 4,
      }),
    );
  }

  function onSidebarKeydown(e) {
    if (e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) {
      openMenuFromKeyboard(e);
      return;
    }
    if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
    if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(e.key)) return;
    const row = e.target instanceof Element ? e.target.closest('.ff-sbrow') : null;
    if (!row || e.target !== row) return;
    const rows = [...e.currentTarget.querySelectorAll('.ff-sbrow[tabindex="0"]')];
    const at = rows.indexOf(row);
    const step = e.key === 'ArrowDown' ? 1 : -1;
    const next = e.key === 'Home' ? rows[0] : e.key === 'End' ? rows[rows.length - 1] : rows[at + step];
    e.preventDefault();
    next?.focus();
  }

  function onPlaylistRowPointerDown(e, playlistId, isFolder = false) {
    if (e.button !== 0 || renamingPlaylistId === playlistId) return;
    let moved = false;
    const dragIds = isMultiSelected(playlistId)
      ? outermostIds(library.playlists, playlistRowOrder.filter((id) => sidebarSelection.ids.includes(id)))
      : [playlistId];
    const stickDroppable = dragIds.length > 1 || !isFolder;
    function onMove(ev) {
      if (!moved && Math.hypot(ev.clientX - e.clientX, ev.clientY - e.clientY) > 6) {
        moved = true;
        playlistDragIds = dragIds;
        dnd.kind = 'playlist';
        dnd.playlistDragIds = dragIds;
        dnd.playlistDragFromStick = null;
      }
      if (moved) {
        updatePointer(ev.clientX, ev.clientY);
        const stick = stickDroppable ? stickTargetAt(ev.clientX, ev.clientY) : { path: null, surface: null };
        dnd.dropStickPath = stick.path;
        dnd.dropStickSurface = stick.surface;
        playlistDrop = playlistDropAt(ev.clientX, ev.clientY);
      }
    }
    async function onUp() {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      if (!moved) return;
      const stickPath = dnd.dropStickPath;
      const drop = playlistDrop;
      playlistDrop = null;
      playlistDragIds = [];
      endDrag();
      if (stickPath) await dropPlaylistsOnStick(stickPath, playlistIdsInSelection(library.playlists, dragIds));
      else if (drop) await applyPlaylistDrop(dragIds, drop);
    }
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="ff-sidebar" role="navigation" aria-label="Library and playlists" style:width="{ui.sidebarWidth}px" onkeydown={onSidebarKeydown}>
  <ResizeHandle
    edge="right"
    label="Resize sidebar"
    value={ui.sidebarWidth}
    min={PANEL_WIDTH.sidebar.min}
    max={PANEL_WIDTH.sidebar.max}
    onResize={(px) => setPanelWidth('sidebar', px)}
    onCommit={() => commitPanelWidth('sidebar')}
    onReset={() => resetPanelWidth('sidebar')}
  />
  <div
    class="ff-sidebar__chrome ff-titlebar-band"
    data-tauri-drag-region
    aria-hidden={getIsTauri()}
  >
    {#if !getIsTauri()}
      <div class="ff-sidebar__traffic" aria-label="Window">
        <span class="ff-sidebar__dot ff-sidebar__dot--close"></span>
        <span class="ff-sidebar__dot ff-sidebar__dot--min"></span>
        <span class="ff-sidebar__dot ff-sidebar__dot--max"></span>
      </div>
    {/if}
    {#if ui.searchMode === 'sidebar'}
      <span class="ff-sidebar__search"><SearchIconButton /></span>
    {/if}
  </div>
  <div class="ff-sidebar__top">
    <SidebarSection
      label={sidebarData.favorites.label}
      onHeaderContextMenu={(e) => openSectionMenu(e, 'favorites')}
    >
      {#each sidebarData.favorites.rows as row}
        {@const pl = library.playlists.find(p => p.name === row.label && !p.is_folder)}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="ff-sidebar__drop-wrap {rowClassExtra(pl?.id)}"
          data-drop-track-playlist-id={pl?.id}
          oncontextmenu={(e) => openSourceMenu(e, row.label, row.label)}
        >
          <SidebarRow
            kind={row.kind}
            digit={row.digit}
            label={row.label}
            count={row.count}
            state={sidebarRowState(row.label, pl?.id)}
            onclick={() => handleRowClick(row.label)}
            onOpenInPanel={pl ? () => dockOrganizeTarget({ kind: 'playlist', id: pl.name, label: pl.name, playlistId: pl.id }) : undefined}
          />
        </div>
      {/each}
      {#if showAddFavorite}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div oncontextmenu={(e) => openSectionMenu(e, 'favorites')}>
          <SidebarRow
            kind="leaf"
            icon="plus"
            label="Add favorite"
            state="rest"
            splitAction={false}
            onclick={handleAddFavorite}
          />
        </div>
      {/if}
    </SidebarSection>

    <SidebarSection
      label={sidebarData.library.label}
      onHeaderContextMenu={(e) => openSectionMenu(e, 'library')}
    >
      {#each sidebarData.library.rows as row}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          oncontextmenu={(e) => openSourceMenu(e, row.label, row.label)}
        >
          <SidebarRow
            kind={row.kind}
            icon={row.icon}
            label={row.label}
            count={row.count}
            state={sidebarRowState(row.label)}
            onclick={() => handleRowClick(row.label)}
          />
        </div>
      {/each}
    </SidebarSection>

    <SidebarSection
      label={sidebarData.playlists.label}
      showAdd
      addDropActive={dnd.dropNewPlaylist}
      onAdd={handleAddPlaylist}
      onHeaderContextMenu={(e) => openSectionMenu(e, 'playlists')}
    >
      {#each shownPlaylistRows as row}
        {@const isFolder = row.kind === 'folder'}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="ff-sidebar__drop-wrap {rowClassExtra(row.id)}"
          data-playlist-id={row.id}
          data-playlist-folder={isFolder ? '' : undefined}
          data-drop-track-playlist-id={isFolder ? undefined : row.id}
          onpointerdown={(e) => onPlaylistRowPointerDown(e, row.id, isFolder)}
          oncontextmenu={(e) => openPlaylistRowMenu(e, row, isFolder)}
        >
          <SidebarRow
            kind={row.kind}
            icon={isFolder ? (row.expanded ? 'folder-open' : 'folder') : undefined}
            label={row.label}
            count={row.count}
            depth={row.depth}
            expanded={row.expanded}
            state={sidebarRowState(row.label, row.id, isFolder)}
            renaming={renamingPlaylistId === row.id}
            bind:renameValue={renameDraft}
            onRenameCommit={() => commitPlaylistRename(row.id)}
            onRenameCancel={cancelPlaylistRename}
            onclick={(e) => handlePlaylistRowClick(e, row, isFolder)}
            ondblclick={() => startPlaylistRename(row.id, row.label)}
            onOpenInPanel={isFolder
              ? undefined
              : () => dockOrganizeTarget({ kind: 'playlist', id: row.label, label: row.label, playlistId: row.id })}
          />
        </div>
      {/each}
    </SidebarSection>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="ff-sidebar__fill {playlistDrop?.id === 0 || dnd.dropNewPlaylist ? 'drop-into' : ''}"
      data-playlist-root-drop
      data-new-playlist-drop
      oncontextmenu={(e) => openSectionMenu(e, 'playlists')}
    ></div>
  </div>

  <div class="ff-sidebar__bottom">
    <SidebarSection
      label={sidebarData.usb.label}
      onHeaderContextMenu={(e) => openSectionMenu(e, 'usb')}
    >
      {#each sidebarData.usb.rows as row}
        {@const stickStatus = statusOfStick(row.path)}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="ff-sidebar__drop-wrap"
          class:drag-over={(dnd.kind === 'playlist' || dnd.kind === 'tracks') && dnd.dropStickPath === row.path && dnd.dropStickSurface === 'row'}
          data-drop-stick-path={row.path}
          data-drop-surface="row"
          oncontextmenu={(e) => openSourceMenu(e, row.path, row.label, true, row.path)}
        >
          <SidebarRow
            kind="device"
            icon="usb"
            label={row.label}
            state={ui.activeSidebarRow === row.path ? 'active' : 'rest'}
            onclick={() => handleRowClick(row.path, true, row.path)}
            onOpenInPanel={() => dockOrganizeTarget({ kind: 'usb', id: row.path, label: row.label })}
          />
          {#if stickStatus.tone === 'problem'}
            <button
              type="button"
              class="ff-sidebar__stick-status ff-sidebar__stick-status--problem"
              data-stick-status={stickStatus.kind}
              aria-label="{row.label}: {stickStatus.label}. Show what is going on"
              title="{stickStatus.label}. Click to see what is going on"
              onclick={(e) => toggleStickProblem(row.path, e.currentTarget)}
            >
              <StickDot tone="problem" label="" />
            </button>
          {:else}
            <span class="ff-sidebar__stick-status" data-stick-status={stickStatus.kind}>
              <StickDot tone={stickStatus.tone} label={stickStatus.label} />
            </span>
          {/if}
        </div>
      {/each}
    </SidebarSection>
  </div>
</div>

{#if contextMenu.open}
  <ContextMenu
    open
    x={contextMenu.x}
    y={contextMenu.y}
    items={contextMenuItems()}
    onClose={closeContextMenu}
  />
{/if}

{#if removeFavoriteDialog != null}
  <Dialog
    open
    title="Remove favorite?"
    bodyText={removeFavoriteDialogBody}
    onClose={() => (removeFavoriteDialog = null)}
  >
    {#snippet actions()}
      <Button
        label="Cancel"
        variant="ghost"
        onclick={() => (removeFavoriteDialog = null)}
      />
      <Button
        label="Remove"
        variant="destructive"
        onclick={() => removeFavoriteDialog && commitRemoveFavorite(removeFavoriteDialog.id)}
      />
    {/snippet}
  </Dialog>
{/if}

<style>
  .ff-sidebar {
    position: relative;
    flex: none;
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--ff-bg);
    border-right: 1px solid var(--ff-border);
  }
  .ff-sidebar__chrome {
    position: relative;
    display: flex;
    align-items: center;
    /* Room for native traffic lights (trafficLightPosition.x ≈ 14px) */
    padding-left: var(--ff-traffic-light-x);
    padding-right: var(--ff-space-6);
    background: var(--ff-bg);
  }
  .ff-sidebar__search {
    position: absolute;
    top: 0;
    right: var(--ff-space-4);
    display: flex;
    align-items: center;
    height: calc(var(--ff-titlebar-search-band-height) - 1px);
  }
  .ff-sidebar__traffic {
    display: flex;
    align-items: center;
    gap: var(--ff-space-3);
  }
  .ff-sidebar__dot {
    width: 10px;
    height: 10px;
    padding: 0;
    border: none;
    border-radius: 50%;
    cursor: default;
  }
  .ff-sidebar__dot--close {
    background: #ff5f57;
  }
  .ff-sidebar__dot--min {
    background: #febc2e;
  }
  .ff-sidebar__dot--max {
    background: #28c840;
  }
  .ff-sidebar__top {
    flex: 1 1 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }
  .ff-sidebar__bottom {
    margin-top: auto;
    border-top: 1px solid var(--ff-border);
  }
  .ff-sidebar__drop-wrap.drag-over :global(.ff-sbrow) {
    outline: 1px solid var(--ff-accent);
    outline-offset: -1px;
    background: var(--ff-hover);
  }
  .ff-sidebar__drop-wrap.drag-source :global(.ff-sbrow) {
    opacity: 0.5;
  }
  .ff-sidebar__drop-wrap.drop-into :global(.ff-sbrow) {
    outline: 1px solid var(--ff-accent);
    outline-offset: -1px;
    background: var(--ff-hover);
  }
  .ff-sidebar__drop-wrap.drop-before :global(.ff-sbrow) {
    box-shadow: inset 0 2px 0 var(--ff-accent);
  }
  .ff-sidebar__drop-wrap.drop-after :global(.ff-sbrow) {
    box-shadow: inset 0 -2px 0 var(--ff-accent);
  }
  .ff-sidebar__drop-wrap:has(.ff-sidebar__stick-status) {
    position: relative;
  }
  .ff-sidebar__stick-status {
    position: absolute;
    top: 0;
    bottom: 0;
    right: calc(var(--ff-space-6) + 6px);
    display: flex;
    align-items: center;
    pointer-events: none;
  }
  .ff-sidebar__drop-wrap:hover .ff-sidebar__stick-status:not(.ff-sidebar__stick-status--problem),
  .ff-sidebar__drop-wrap:focus-within .ff-sidebar__stick-status:not(.ff-sidebar__stick-status--problem) {
    display: none;
  }
  .ff-sidebar__stick-status--problem {
    top: 3px;
    bottom: 3px;
    right: calc(var(--ff-space-6) + 2px);
    justify-content: center;
    width: 20px;
    padding: 0;
    border: none;
    border-radius: var(--ff-radius-md);
    background: transparent;
    cursor: pointer;
    pointer-events: auto;
  }
  .ff-sidebar__stick-status--problem:hover {
    background: var(--ff-elev-hi);
  }
  .ff-sidebar__stick-status--problem:focus-visible {
    outline: 1px solid var(--ff-accent);
  }
  .ff-sidebar__drop-wrap:hover .ff-sidebar__stick-status--problem,
  .ff-sidebar__drop-wrap:has(:global(.ff-sbrow:focus-within)) .ff-sidebar__stick-status--problem {
    right: calc(var(--ff-space-6) + 22px);
  }
  .ff-sidebar__fill {
    flex: 1 0 24px;
  }
  .ff-sidebar__fill.drop-into {
    box-shadow: inset 0 2px 0 var(--ff-accent);
  }
</style>
