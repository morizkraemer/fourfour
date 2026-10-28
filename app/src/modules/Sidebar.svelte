<!--
  Sidebar.svelte — Left navigation with playlist drop targets and row reorder.
-->
<script>
  import { onMount, onDestroy } from 'svelte';
  import { SidebarSection, SidebarRow, ContextMenu, Dialog, Button, ResizeHandle } from '$ds';
  import { getIsTauri } from '../services/tauri.svelte.ts';
  import {
    buildSourceContextMenuItems,
    buildSidebarSectionMenuItems,
    buildFolderContextMenuItems,
  } from '../menus/context-menus.ts';
  import {
    addPlaylist,
    renamePlaylist,
    refreshVolumes,
    selectVolume,
    ejectVolume,
    library,
    movePlaylistInTree,
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
  import { dnd, endDrag } from '../stores/dnd.svelte.ts';
  import { dropPlaylistOnStick } from '../stores/organize-drop.svelte.ts';
  import { toggleFolder } from '../stores/sidebar-folders.svelte.ts';

  let intervalId;
  let renamingPlaylistId = $state(null);
  let renameDraft = $state('');
  let playlistDragId = $state(null);
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
  });

  onMount(() => {
    refreshVolumes();
    intervalId = setInterval(refreshVolumes, 5000);
  });

  onDestroy(() => {
    if (intervalId) clearInterval(intervalId);
  });

  function handleRowClick(id, isUsb = false, path = null) {
    ui.activeSidebarRow = id;
    if (isUsb) {
      selectVolume(path);
    } else {
      selectVolume(null);
    }
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
    if (!isFolder && ui.activeSidebarRow === label) return 'active';
    if (playlistId != null && dnd.dropPlaylistId === playlistId) return 'selected';
    if (playlistId != null && playlistDragId === playlistId) return 'hover';
    return 'rest';
  }

  function rowClassExtra(playlistId) {
    if (playlistId != null && dnd.dropPlaylistId === playlistId) return 'drag-over';
    if (playlistId != null && playlistDragId === playlistId) return 'drag-source';
    if (playlistId != null && playlistDrop?.id === playlistId) return `drop-${playlistDrop.placement}`;
    return '';
  }

  function onPlaylistPointerEnter(playlistId, isFolder = false) {
    if (dnd.kind === 'tracks' && dnd.trackIds.length > 0 && !isFolder) {
      dnd.dropPlaylistId = playlistId;
      dnd.dropPanelSourceId = null;
      dnd.dropRowIndex = null;
    }
  }

  function onPlaylistPointerLeave(playlistId) {
    if (dnd.dropPlaylistId === playlistId) dnd.dropPlaylistId = null;
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

  async function applyPlaylistDrop(dragId, drop) {
    if (drop.id === 0) return movePlaylistInTree(dragId, 0);
    const target = library.playlists.find((p) => p.id === drop.id);
    if (!target || target.id === dragId) return false;
    if (drop.placement === 'into') return movePlaylistInTree(dragId, target.id);
    return movePlaylistInTree(dragId, target.parent_id, target.id, drop.placement === 'after');
  }

  function closeContextMenu() {
    contextMenu = { ...contextMenu, open: false, section: null };
  }

  function contextMenuItems() {
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

  function onPlaylistRowPointerDown(e, playlistId, isFolder = false) {
    if (e.button !== 0 || renamingPlaylistId === playlistId) return;
    let moved = false;
    function onMove(ev) {
      if (!moved && Math.hypot(ev.clientX - e.clientX, ev.clientY - e.clientY) > 6) {
        moved = true;
        playlistDragId = playlistId;
        dnd.kind = 'playlist';
      }
      if (moved) {
        const stick = isFolder
          ? null
          : document.elementFromPoint(ev.clientX, ev.clientY)?.closest('[data-drop-stick-path]');
        dnd.dropStickPath = stick ? stick.getAttribute('data-drop-stick-path') : null;
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
      playlistDragId = null;
      endDrag();
      if (stickPath) await dropPlaylistOnStick(stickPath, playlistId);
      else if (drop) await applyPlaylistDrop(playlistId, drop);
    }
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
  }
</script>

<div class="ff-sidebar" style:width="{ui.sidebarWidth}px">
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
  </div>
  <div class="ff-sidebar__top">
    <SidebarSection
      label={library.sidebarData.favorites.label}
      onHeaderContextMenu={(e) => openSectionMenu(e, 'favorites')}
    >
      {#each library.sidebarData.favorites.rows as row}
        {@const pl = library.playlists.find(p => p.name === row.label && !p.is_folder)}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="ff-sidebar__drop-wrap {rowClassExtra(pl?.id)}"
          onpointerenter={() => pl && onPlaylistPointerEnter(pl.id)}
          onpointerleave={() => pl && onPlaylistPointerLeave(pl.id)}
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
      label={library.sidebarData.library.label}
      onHeaderContextMenu={(e) => openSectionMenu(e, 'library')}
    >
      {#each library.sidebarData.library.rows as row}
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
      label={library.sidebarData.playlists.label}
      showAdd
      addDropActive={dnd.dropNewPlaylist}
      onAdd={handleAddPlaylist}
      onHeaderContextMenu={(e) => openSectionMenu(e, 'playlists')}
    >
      {#each library.sidebarData.playlists.rows as row}
        {@const isFolder = row.kind === 'folder'}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="ff-sidebar__drop-wrap {rowClassExtra(row.id)}"
          data-playlist-id={row.id}
          data-playlist-folder={isFolder ? '' : undefined}
          onpointerenter={() => onPlaylistPointerEnter(row.id, isFolder)}
          onpointerleave={() => onPlaylistPointerLeave(row.id)}
          onpointerdown={(e) => onPlaylistRowPointerDown(e, row.id, isFolder)}
          oncontextmenu={(e) => (isFolder ? openFolderMenu(e, row.id) : openSourceMenu(e, row.label, row.label))}
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
            onclick={() => (isFolder ? toggleFolder(row.id) : handleRowClick(row.label))}
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
      class="ff-sidebar__fill {playlistDrop?.id === 0 ? 'drop-into' : ''}"
      data-playlist-root-drop
      oncontextmenu={(e) => openSectionMenu(e, 'playlists')}
    ></div>
  </div>

  <div class="ff-sidebar__bottom">
    <SidebarSection
      label={library.sidebarData.usb.label}
      onHeaderContextMenu={(e) => openSectionMenu(e, 'usb')}
    >
      {#each library.sidebarData.usb.rows as row}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="ff-sidebar__drop-wrap"
          class:drag-over={(dnd.kind === 'playlist' || dnd.kind === 'tracks') && dnd.dropStickPath === row.path}
          data-drop-stick-path={row.path}
          oncontextmenu={(e) => openSourceMenu(e, row.path, row.label, true, row.path)}
        >
          <SidebarRow
            kind={row.kind}
            label={row.label}
            status={row.status}
            state={ui.activeSidebarRow === row.path ? 'active' : 'rest'}
            onclick={() => handleRowClick(row.path, true, row.path)}
            onOpenInPanel={() => dockOrganizeTarget({ kind: 'usb', id: row.path, label: row.label })}
          />
        </div>
      {/each}
    </SidebarSection>
  </div>
</div>

<ContextMenu
  open={contextMenu.open}
  x={contextMenu.x}
  y={contextMenu.y}
  items={contextMenuItems()}
  onClose={closeContextMenu}
/>

<Dialog
  open={removeFavoriteDialog != null}
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
    display: flex;
    align-items: center;
    /* Room for native traffic lights (trafficLightPosition.x ≈ 14px) */
    padding-left: var(--ff-traffic-light-x);
    padding-right: var(--ff-space-6);
    background: var(--ff-bg);
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
  .ff-sidebar__fill {
    flex: 1 0 24px;
  }
  .ff-sidebar__fill.drop-into {
    box-shadow: inset 0 2px 0 var(--ff-accent);
  }
</style>
