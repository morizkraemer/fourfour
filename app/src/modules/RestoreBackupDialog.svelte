<script>
  import { Dialog, Button, StatusDot, ProgressBar } from '$ds';
  import { backup, cancelRestore, confirmRestore } from '../stores/backup.svelte.ts';
  import { progressText, restoreLines, restoreSummary } from '../lib/backup.ts';

  let preview = $derived(backup.preview);
  let percent = $derived(backup.progress && backup.progress.total > 0 ? (backup.progress.done / backup.progress.total) * 100 : 0);
</script>

<Dialog open={preview !== null} title="Restore this backup?" onClose={cancelRestore}>
  {#snippet body()}
    {#if preview}
      <div class="ff-restore" data-restore-dialog>
        <p class="ff-restore__sum">{restoreSummary(preview)}</p>
        <div class="ff-restore__lines">
          {#each restoreLines(preview) as line (line.text)}
            <div class="ff-restore__line">
              <StatusDot status={line.status} />
              <span class="ff-restore__text">{line.text}</span>
            </div>
          {/each}
        </div>
        {#if backup.restoring}
          <p class="ff-restore__note" data-restore-progress>
            {backup.progress ? `Restoring · ${progressText(backup.progress.done, backup.progress.total)}` : 'Restoring…'}
          </p>
          <ProgressBar value={percent} />
        {/if}
        {#if backup.restoreError}
          <p class="ff-restore__error">{backup.restoreError}</p>
        {/if}
      </div>
    {/if}
  {/snippet}
  {#snippet actions()}
    <Button label="Cancel" variant="ghost" disabled={backup.restoring} onclick={cancelRestore} />
    <Button
      label={preview?.includes_audio ? 'Restore…' : 'Restore'}
      variant="primary"
      disabled={backup.restoring}
      onclick={() => void confirmRestore()}
    />
  {/snippet}
</Dialog>

<style>
  .ff-restore {
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-4);
  }
  .ff-restore p {
    margin: 0;
  }
  .ff-restore__sum {
    color: var(--ff-text);
  }
  .ff-restore__lines {
    display: flex;
    flex-direction: column;
  }
  .ff-restore__line {
    display: flex;
    align-items: center;
    gap: var(--ff-space-4);
    min-height: 28px;
  }
  .ff-restore__text {
    flex: 1 1 0;
    min-width: 0;
  }
  .ff-restore__note {
    color: var(--ff-muted);
  }
  .ff-restore__error {
    color: var(--ff-danger);
  }
</style>
