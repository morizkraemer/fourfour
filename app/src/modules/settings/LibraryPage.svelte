<script>
  import { SettingRow, Button } from '$ds';
  import { library, changeLocalLibraryPath } from '../../stores/library.svelte.ts';
  import { pickDirectory, getIsTauri } from '../../services/tauri.svelte.ts';
  import { openRekordboxImport } from '../../stores/rekordbox-import.svelte.ts';

  let busy = $derived(library.analyzing || library.syncing);

  async function changeLibrary() {
    const dir = await pickDirectory();
    if (dir) await changeLocalLibraryPath(dir);
  }
</script>

<div class="ff-settings-card ff-settings-card--status">
  <div class="stats">
    <b>{library.trackCount} tracks</b>
    <span>{library.playlists.length} playlists</span>
  </div>
  <span class="path" title={library.libraryPath}>{library.libraryPath || 'Not set'}</span>
</div>

<div class="ff-settings-section">Database</div>
<div class="ff-settings-card">
  <SettingRow title="Library database" description="Tracks, playlists and the analysis cache live next to this file.">
    {#snippet control()}
      <Button label="Change…" size="small" disabled={busy} onclick={changeLibrary} />
    {/snippet}
  </SettingRow>
</div>

<div class="ff-settings-section">Import</div>
<div class="ff-settings-card">
  <SettingRow
    title="Import from Rekordbox"
    description="Tracks, playlists, beat grids and cues from Rekordbox, a Pioneer USB stick or rekordbox.xml."
  >
    {#snippet control()}
      <Button label="Import…" size="small" disabled={busy || !getIsTauri()} onclick={openRekordboxImport} />
    {/snippet}
  </SettingRow>
</div>

<style>
  .stats {
    display: flex;
    align-items: baseline;
    gap: 10px;
    font-family: var(--ff-font);
    font-size: 13px;
  }

  .stats span {
    font-size: 12px;
    color: var(--ff-muted);
  }

  .path {
    font-family: var(--ff-font-mono);
    font-size: 11.5px;
    color: var(--ff-text-mid);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
