<!--
  Sidebar.svelte — Left navigation with playlist drop targets and row reorder.
-->
<script>
  import { onMount, onDestroy } from 'svelte';
  import { SidebarSection, SidebarRow, ContextMenu, Dialog, Button } from '$ds';
  import { getIsTauri } from '../services/tauri.svelte.ts';
  import {
    buildSourceContextMenuItems,
    buildSidebarSectionMenuItems,
  } from '../menus/context-menus.ts';
  import {
    addPlaylist,
    defaultPlaylistName,
    renamePlaylist,
    refreshVolumes,
    selectVolume,
    ejectVolume,
    library,
    reorderPlaylists,
    favoritePlaylistName,
    nextFavoriteSlot,
    canAddFavorite,
    removeFavoriteSlot,
    favoriteSlotFromName,
    canRemoveFavoriteSlot,
  } from '../stores/library.svelte.ts';
  import { ui, dockOrganizeTarget } from '../stores/ui.svelte.ts';
  import { sidebarSourceLabel } from '../stores/library.svelte.ts';
  import { dnd, endDrag } from '../stores/dnd.svelte.ts';

  let intervalId;
  let renamingPlaylistId = $state(null);
  let renameDraft = $state('');
  let playlistDragId = $state(null);
  let playlistDropId = $state(null);
  let contextMenu = $state({
    open: false,
    x: 0,
    y: 0,
    kind: 'source',
    sourceId: '',
    label: '',
    section: null,
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
    const name = defaultPlaylistName();
    const playlistId = await addPlaylist(name);
    handleRowClick(name);
    dockOrganizeTarget({ kind: 'playlist', id: name, label: name, playlistId });
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
  }

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

  function sidebarRowState(label, playlistId = null) {
    if (ui.activeSidebarRow === label) return 'active';
    if (playlistId != null && dnd.dropPlaylistId === playlistId) return 'selected';
    if (playlistId != null && playlistDragId === playlistId) return 'hover';
    return 'rest';
  }

  function rowClassExtra(playlistId) {
    if (playlistId != null && dnd.dropPlaylistId === playlistId) return 'drag-over';
    if (playlistId != null && playlistDragId === playlistId) return 'drag-source';
    return '';
  }

  function onPlaylistPointerEnter(playlistId) {
    if (dnd.kind === 'tracks' && dnd.trackIds.length > 0) {
      dnd.dropPlaylistId = playlistId;
      dnd.dropPanelSourceId = null;
      dnd.dropRowIndex = null;
    }
    if (dnd.kind === 'playlist' || playlistDragId != null) {
      playlistDropId = playlistId;
    }
  }

  function onPlaylistPointerLeave(playlistId) {
    if (dnd.dropPlaylistId === playlistId) dnd.dropPlaylistId = null;
    if (playlistDropId === playlistId) playlistDropId = null;
  }

  async function onPlaylistPointerUp() {
    if (playlistDragId != null && playlistDropId != null && playlistDragId !== playlistDropId) {
      await reorderPlaylists(playlistDragId, playlistDropId);
    }
    playlistDragId = null;
    playlistDropId = null;
  }

  function closeContextMenu() {
    contextMenu = { ...contextMenu, open: false, section: null };
  }

  function contextMenuItems() {
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

  function onPlaylistRowPointerDown(e, playlistId) {
    if (e.button !== 0 || renamingPlaylistId === playlistId) return;
    let moved = false;
    function onMove(ev) {
      if (!moved && Math.abs(ev.clientX - e.clientX) > 6) {
        moved = true;
        playlistDragId = playlistId;
        dnd.kind = 'playlist';
      }
    }
    function onUp() {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      if (!moved) return;
      onPlaylistPointerUp(playlistId);
      playlistDragId = null;
      playlistDropId = null;
      endDrag();
    }
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
  }
</script>

<div class="ff-sidebar">
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
        {@const pl = library.playlists.find(p => p.name === row.label)}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="ff-sidebar__drop-wrap {rowClassExtra(pl?.id)}"
          onpointerenter={() => pl && onPlaylistPointerEnter(pl.id)}
          onpointerleave={() => pl && onPlaylistPointerLeave(pl.id)}
          onpointerup={onPlaylistPointerUp}
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
      onAdd={handleAddPlaylist}
      onHeaderContextMenu={(e) => openSectionMenu(e, 'playlists')}
    >
      {#each library.sidebarData.playlists.rows as row}
        {@const pl = library.playlists.find(p => p.id === row.id)}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="ff-sidebar__drop-wrap {rowClassExtra(row.id)}"
          onpointerenter={() => onPlaylistPointerEnter(row.id)}
          onpointerleave={() => onPlaylistPointerLeave(row.id)}
          onpointerup={onPlaylistPointerUp}
          onpointerdown={(e) => onPlaylistRowPointerDown(e, row.id)}
          oncontextmenu={(e) => openSourceMenu(e, row.label, row.label)}
        >
          <SidebarRow
            kind={row.kind}
            label={row.label}
            count={row.count}
            state={sidebarRowState(row.label, row.id)}
            renaming={renamingPlaylistId === row.id}
            bind:renameValue={renameDraft}
            onRenameCommit={() => commitPlaylistRename(row.id)}
            onRenameCancel={cancelPlaylistRename}
            onclick={() => handleRowClick(row.label)}
            ondblclick={() => startPlaylistRename(row.id, row.label)}
            onOpenInPanel={() => dockOrganizeTarget({ kind: 'playlist', id: row.label, label: row.label, playlistId: row.id })}
          />
        </div>
      {/each}
    </SidebarSection>
  </div>

  <div class="ff-sidebar__bottom">
    <SidebarSection
      label={library.sidebarData.usb.label}
      onHeaderContextMenu={(e) => openSectionMenu(e, 'usb')}
    >
      {#each library.sidebarData.usb.rows as row}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
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
    display: flex;
    flex-direction: column;
    width: 240px;
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
</style>
