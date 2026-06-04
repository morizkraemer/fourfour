<!--
  Sidebar.svelte — Left navigation sidebar module.
  Traffic lights, nav sections (Favorites/Library/Playlists), USB pinned at bottom.
  Matches module-sidebar.artboard.js structure.
-->
<script>
  import { onMount, onDestroy } from 'svelte';
  import { SidebarSection, SidebarRow } from '$ds';
  import {
    addPlaylist,
    deletePlaylist,
    refreshVolumes,
    selectVolume,
    ejectVolume,
    library
  } from '../stores/library.svelte.ts';
  import { ui } from '../stores/ui.svelte.ts';

  let dialogEl;
  let newPlaylistName = '';
  let intervalId;

  onMount(() => {
    // Refresh volumes immediately
    refreshVolumes();
    // Poll for volume changes every 5 seconds
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

  function handleAddPlaylist() {
    newPlaylistName = '';
    dialogEl?.showModal();
  }

  function submitPlaylist() {
    const name = newPlaylistName.trim();
    if (name) {
      addPlaylist(name);
    }
    dialogEl?.close();
  }
</script>

<div class="ff-sidebar">
  <!-- macOS traffic lights -->
  <div class="ff-sidebar__chrome">
    <span class="ff-sidebar__dot ff-sidebar__dot--close"></span>
    <span class="ff-sidebar__dot ff-sidebar__dot--min"></span>
    <span class="ff-sidebar__dot ff-sidebar__dot--max"></span>
  </div>

  <!-- scrollable sections -->
  <div class="ff-sidebar__top">
    <SidebarSection label={library.sidebarData.favorites.label}>
      {#each library.sidebarData.favorites.rows as row}
        <SidebarRow
          kind={row.kind}
          digit={row.digit}
          label={row.label}
          count={row.count}
          state={ui.activeSidebarRow === row.label ? 'active' : 'rest'}
          splitAction={false}
          onclick={() => handleRowClick(row.label)}
        />
      {/each}
    </SidebarSection>

    <SidebarSection label={library.sidebarData.library.label}>
      {#each library.sidebarData.library.rows as row}
        <SidebarRow
          kind={row.kind}
          icon={row.icon}
          label={row.label}
          count={row.count}
          state={ui.activeSidebarRow === row.label ? 'active' : 'rest'}
          splitAction={false}
          onclick={() => handleRowClick(row.label)}
        />
      {/each}
    </SidebarSection>

    <SidebarSection label={library.sidebarData.playlists.label} showAdd onclick={handleAddPlaylist}>
      {#each library.sidebarData.playlists.rows as row}
        <SidebarRow
          kind={row.kind}
          label={row.label}
          count={row.count}
          state={ui.activeSidebarRow === row.label ? 'active' : 'rest'}
          splitAction={true}
          onSplit={() => deletePlaylist(row.id)}
          onclick={() => handleRowClick(row.label)}
        />
      {/each}
    </SidebarSection>
  </div>

  <!-- USB pinned at bottom -->
  <div class="ff-sidebar__bottom">
    <SidebarSection label={library.sidebarData.usb.label}>
      {#each library.sidebarData.usb.rows as row}
        <SidebarRow
          kind={row.kind}
          label={row.label}
          status={row.status}
          state={ui.activeSidebarRow === row.path ? 'active' : 'rest'}
          splitAction={true}
          onSplit={() => ejectVolume(row.path)}
          onclick={() => handleRowClick(row.path, true, row.path)}
        />
      {/each}
    </SidebarSection>
  </div>
</div>

<!-- Playlist creation dialog -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<dialog bind:this={dialogEl} class="ff-playlist-dialog" onclick={(e) => e.target === dialogEl && dialogEl.close()}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="ff-playlist-dialog__content" onclick={(e) => e.stopPropagation()}>
    <h3>Create Playlist</h3>
    <input
      type="text"
      placeholder="Playlist Name"
      bind:value={newPlaylistName}
      onkeydown={(e) => e.key === 'Enter' && submitPlaylist()}
    />
    <div class="ff-playlist-dialog__actions">
      <button class="ff-dialog-btn ff-dialog-btn--cancel" onclick={() => dialogEl.close()}>Cancel</button>
      <button class="ff-dialog-btn ff-dialog-btn--create" onclick={submitPlaylist}>Create</button>
    </div>
  </div>
</dialog>

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
    gap: var(--ff-space-3);
    padding: var(--ff-space-4) var(--ff-space-6);
    -webkit-app-region: drag;
  }
  .ff-sidebar__dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }
  .ff-sidebar__dot--close { background: #ff5f57; }
  .ff-sidebar__dot--min   { background: #febc2e; }
  .ff-sidebar__dot--max   { background: #28c840; }

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

  /* Simple styling for the playlist dialog matching system colors */
  .ff-playlist-dialog {
    border: 1px solid var(--ff-border);
    background: var(--ff-bg);
    border-radius: var(--ff-radius-lg);
    color: var(--ff-text);
    padding: 0;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
  }
  .ff-playlist-dialog::backdrop {
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(2px);
  }
  .ff-playlist-dialog__content {
    padding: var(--ff-space-6);
    width: 280px;
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-4);
  }
  .ff-playlist-dialog__content h3 {
    margin: 0;
    font-size: var(--ff-type-title);
    font-weight: 500;
  }
  .ff-playlist-dialog__content input {
    width: 100%;
    padding: var(--ff-space-3);
    background: var(--ff-surface);
    border: 1px solid var(--ff-border);
    border-radius: var(--ff-radius-md);
    color: var(--ff-text);
    font-size: var(--ff-type-compact);
  }
  .ff-playlist-dialog__content input:focus {
    outline: 1px solid var(--ff-border);
  }
  .ff-playlist-dialog__actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--ff-space-3);
    margin-top: var(--ff-space-2);
  }
  .ff-dialog-btn {
    padding: var(--ff-space-2) var(--ff-space-4);
    border-radius: var(--ff-radius-md);
    font-size: var(--ff-type-compact);
    cursor: pointer;
    border: none;
  }
  .ff-dialog-btn--cancel {
    background: transparent;
    color: var(--ff-muted);
  }
  .ff-dialog-btn--cancel:hover {
    color: var(--ff-text);
  }
  .ff-dialog-btn--create {
    background: var(--ff-text);
    color: var(--ff-bg);
    font-weight: 500;
  }
  .ff-dialog-btn--create:hover {
    opacity: 0.9;
  }
</style>
