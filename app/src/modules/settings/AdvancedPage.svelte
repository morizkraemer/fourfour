<script>
  import { SettingRow, Button, Dialog } from '$ds';
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
  <SettingRow
    title="Reset library"
    description="Deletes library.db, its WAL files and the analysis cache. Tracks and playlists are wiped; audio files stay."
  >
    {#snippet control()}
      <Button label="Reset library…" variant="destructive" size="small" disabled={busy || resetting} onclick={() => (confirmOpen = true)} />
    {/snippet}
  </SettingRow>
</div>

<Dialog
  open={confirmOpen}
  title="Reset library database?"
  bodyText="This permanently deletes all tracks, playlists, and analysis cache for the current library file. Audio files on disk are not removed. This cannot be undone."
  onClose={() => {
    if (!resetting) confirmOpen = false;
  }}
>
  {#snippet actions()}
    <Button label="Cancel" variant="ghost" disabled={resetting} onclick={() => (confirmOpen = false)} />
    <Button
      label={resetting ? 'Resetting…' : 'Reset'}
      variant="destructive"
      disabled={resetting}
      onclick={() => void commitReset()}
    />
  {/snippet}
</Dialog>

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
