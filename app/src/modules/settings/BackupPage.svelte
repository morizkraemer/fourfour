<script>
  import { SettingRow, SegmentedControl, Button, ProgressBar } from '$ds';
  import { library } from '../../stores/library.svelte.ts';
  import { getIsTauri } from '../../services/tauri.svelte.ts';
  import { backup, loadBackupStatus, saveBackup, chooseRestoreFile } from '../../stores/backup.svelte.ts';
  import { includeText, lastBackupText, progressText } from '../../lib/backup.ts';

  const INCLUDE = ['Library + analysis', 'Also audio files'];

  let desktop = getIsTauri();
  let busy = $derived(backup.exporting || backup.restoring || library.analyzing || library.syncing);
  let percent = $derived(backup.progress && backup.progress.total > 0 ? (backup.progress.done / backup.progress.total) * 100 : 0);

  $effect(() => {
    void loadBackupStatus();
  });

  let saveDescription = $derived.by(() => {
    if (backup.exporting) return backup.progress ? progressText(backup.progress.done, backup.progress.total) : 'Saving backup…';
    return backup.exportError || lastBackupText(backup.status?.last ?? null);
  });
</script>

<div class="ff-settings-section">Back up</div>
<div class="ff-settings-card" data-backup-save>
  <SettingRow title="Include" description={includeText(backup.status?.sizes ?? null)}>
    {#snippet control()}
      <SegmentedControl
        items={INCLUDE}
        activeIndex={backup.includeAudio ? 1 : 0}
        disabled={backup.exporting}
        onChange={(i) => (backup.includeAudio = i === 1)}
      />
    {/snippet}
  </SettingRow>
  <SettingRow title="Back up now" description={saveDescription}>
    {#snippet control()}
      <Button label="Save backup…" size="small" disabled={busy || !desktop} onclick={() => void saveBackup()} />
    {/snippet}
  </SettingRow>
  {#if backup.exporting}
    <ProgressBar value={percent} />
  {/if}
</div>

<div class="ff-settings-section">Restore</div>
<div class="ff-settings-card" data-backup-restore>
  <SettingRow
    title="Restore from a backup"
    description={backup.restoreError && !backup.preview
      ? backup.restoreError
      : 'Replaces library and settings. Your current library is saved as a backup first.'}
  >
    {#snippet control()}
      <Button label="Choose file…" size="small" disabled={busy || !desktop} onclick={() => void chooseRestoreFile()} />
    {/snippet}
  </SettingRow>
</div>
