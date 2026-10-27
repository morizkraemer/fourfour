<script>
  import { SettingRow, Button } from '$ds';
  import { library, resetLibraryDatabase } from '../../stores/library.svelte.ts';

  let busy = $derived(library.analyzing || library.syncing);
  let confirmOpen = $state(false);
  let resetting = $state(false);

  async function commitReset() {
    resetting = true;
    try {
      await resetLibraryDatabase();
      confirmOpen = false;
    } catch {
      confirmOpen = false;
    } finally {
      resetting = false;
    }
  }
</script>

<div class="ff-settings-section">About</div>
<div class="ff-settings-card">
  <SettingRow title="App version" description="Running build of fourfour.">
    {#snippet control()}<span class="plain">{library.appVersion || '—'}</span>{/snippet}
  </SettingRow>
  <SettingRow title="IPC diagnostic" description="Last Tauri invoke metadata, for connection issues.">
    {#snippet control()}<span class="mono">{library.ipcDiagnostic || '—'}</span>{/snippet}
  </SettingRow>
  {#if library.tauriError}
    <SettingRow title="Last error" description="Most recent backend error shown to the UI.">
      {#snippet control()}<span class="mono">{library.tauriError}</span>{/snippet}
    </SettingRow>
  {/if}
</div>

<div class="ff-settings-section">Danger zone</div>
<div class="ff-settings-card ff-settings-card--danger">
  {#if confirmOpen}
    <SettingRow
      title="Delete every track, playlist and analysis?"
      description="This cannot be undone. Audio files on disk are not removed."
    >
      {#snippet control()}
        <Button label="Cancel" variant="ghost" size="small" disabled={resetting} onclick={() => (confirmOpen = false)} />
        <Button
          label={resetting ? 'Resetting…' : 'Reset library'}
          variant="destructive"
          size="small"
          disabled={resetting}
          onclick={() => void commitReset()}
        />
      {/snippet}
    </SettingRow>
  {:else}
    <SettingRow
      title="Reset library"
      description="Deletes library.db, its WAL files and the analysis cache. Tracks and playlists are wiped; audio files stay."
    >
      {#snippet control()}
        <Button label="Reset library…" variant="destructive" size="small" disabled={busy} onclick={() => (confirmOpen = true)} />
      {/snippet}
    </SettingRow>
  {/if}
</div>

<style>
  .plain,
  .mono {
    font-size: 12px;
    color: var(--ff-text-mid);
    word-break: break-all;
    text-align: right;
  }

  .mono {
    font-family: var(--ff-font-mono);
    font-size: 11px;
  }
</style>
