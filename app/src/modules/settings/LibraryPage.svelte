<script>
  import { SettingRow, Button, Switch } from '$ds';
  import { appSettings, setAnalyzeAfterImport } from '../../stores/app-settings.svelte.ts';
  import { library, changeLocalLibraryPath } from '../../stores/library.svelte.ts';
  import { pickDirectory, getIsTauri } from '../../services/tauri.svelte.ts';
  import { openRekordboxImport } from '../../stores/rekordbox-import.svelte.ts';
  import { fileStatus } from '../../stores/file-status.svelte.ts';
  import { checkLibraryAgain, openHealthDialog } from '../../stores/file-health.svelte.ts';
  import { lastCheckText } from '../../lib/fileHealth.ts';

  let busy = $derived(library.analyzing || library.syncing);

  async function changeLibrary() {
    const dir = await pickDirectory();
    if (dir) {
      await changeLocalLibraryPath(dir);
      void checkLibraryAgain();
    }
  }
</script>

<div class="ff-settings-card ff-settings-card--status">
  <div class="stats">
    <b>{library.trackCount} tracks</b>
    <span>{library.playlists.filter((p) => !p.is_folder).length} playlists</span>
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

<div class="ff-settings-section">Files</div>
<div class="ff-settings-card" data-settings-file-check>
  <SettingRow
    title="Check files"
    description={fileStatus.running ? 'Checking every track’s file…' : lastCheckText(fileStatus.checkedAt, fileStatus)}
  >
    {#snippet control()}
      {#if fileStatus.missing + fileStatus.error > 0}
        <Button label="Show results" size="small" variant="ghost" onclick={openHealthDialog} />
      {/if}
      <Button label="Check now" size="small" disabled={fileStatus.running} onclick={() => void checkLibraryAgain()} />
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
  <SettingRow
    title="Always analyze new tracks automatically"
    description="Analysis starts right after an import, without asking. Off: you are asked each time."
  >
    {#snippet control()}
      <Switch
        label="Always analyze new tracks automatically"
        on={appSettings.analyzeAfterImport}
        onChange={(on) => void setAnalyzeAfterImport(on)}
      />
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
