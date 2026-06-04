<!--
  GlobalHeader.svelte — Top header bar with search field and actions.
  Matches the v3 artboard layout with action controls added for Tauri integrations.
-->
<script>
  import { Icon, Kbd, Button } from '$ds';
  import {
    library,
    importDirectory,
    analyzeTracks,
    setTestCuesOnSelected,
    removeTracks,
    changeLocalLibraryPath,
    syncToUsb,
    wipeVolume
  } from '../stores/library.svelte.ts';
  import { pickDirectory } from '../services/tauri.svelte.ts';
  import { selection, clear as clearSelection } from '../stores/selection.svelte.ts';

  async function handleImport() {
    const dir = await pickDirectory();
    if (dir) {
      await importDirectory(dir);
    }
  }

  async function handleChangeLibrary() {
    const dir = await pickDirectory();
    if (dir) {
      await changeLocalLibraryPath(dir);
    }
  }

  async function handleAnalyze() {
    await analyzeTracks();
  }

  async function handleSetTestCues() {
    const ids = Array.from(selection.ids);
    if (ids.length > 0) {
      await setTestCuesOnSelected(ids);
    }
  }

  async function handleDeleteSelected() {
    const ids = Array.from(selection.ids);
    if (ids.length > 0) {
      if (confirm(`Are you sure you want to delete ${ids.length} selected track(s)?`)) {
        await removeTracks(ids);
        clearSelection();
      }
    }
  }

  async function handleSyncToUsb() {
    if (confirm(`Sync all playlists to ${library.activeVolume}?`)) {
      await syncToUsb();
    }
  }

  async function handleWipeUsb() {
    if (confirm(`⚠ WARNING: Wipe all Pioneer library contents from ${library.activeVolume}? This cannot be undone.`)) {
      await wipeVolume(library.activeVolume);
    }
  }
</script>

<div class="ff-gheader">
  <div class="ff-gheader__search">
    <Icon name="search" size={12} />
    <span class="ff-gheader__placeholder">Search tracks, artists, labels…</span>
    <Kbd key="⌘F" />
  </div>

  <div class="ff-gheader__actions">
    {#if library.activeVolume}
      <Button
        label="Sync to USB"
        variant="primary"
        size="small"
        disabled={library.analyzing || library.syncing || library.playlists.length === 0}
        onclick={handleSyncToUsb}
      />
      <Button
        label="Wipe USB"
        variant="destructive"
        size="small"
        disabled={library.analyzing || library.syncing}
        onclick={handleWipeUsb}
      />
    {:else}
      <Button
        label="Library Path"
        icon="settings"
        size="small"
        disabled={library.analyzing || library.syncing}
        onclick={handleChangeLibrary}
      />
      <Button
        label="Import Folder"
        icon="plus"
        variant="primary"
        size="small"
        disabled={library.analyzing || library.syncing}
        onclick={handleImport}
      />
      <Button
        label="Analyze All"
        size="small"
        disabled={library.tracks.length === 0 || library.analyzing || library.syncing}
        onclick={handleAnalyze}
      />
    {/if}
    {#if selection.ids.size > 0}
      <Button
        label="Set Cues"
        size="small"
        disabled={library.analyzing || library.syncing}
        onclick={handleSetTestCues}
      />
      <Button
        label="Delete"
        variant="destructive"
        size="small"
        disabled={library.analyzing || library.syncing}
        onclick={handleDeleteSelected}
      />
    {/if}
  </div>
</div>

<style>
  .ff-gheader {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: var(--ff-space-4) var(--ff-space-7);
    background: var(--ff-bg);
    border-bottom: 1px solid var(--ff-border);
  }
  .ff-gheader__search {
    display: flex;
    align-items: center;
    gap: var(--ff-space-4);
    width: 420px;
    height: 28px;
    padding: 0 10px;
    background: var(--ff-surface);
    border: 1px solid var(--ff-border);
    border-radius: var(--ff-radius-md);
    color: var(--ff-muted);
  }
  .ff-gheader__placeholder {
    flex: 1 1 0;
    font-family: var(--ff-font);
    font-size: var(--ff-type-compact);
    color: var(--ff-faint);
  }
  .ff-gheader__actions {
    display: flex;
    align-items: center;
    gap: var(--ff-space-3);
  }
</style>
