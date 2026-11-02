<script>
  import { EmptyState, Button } from '$ds';
  import { library } from '../stores/library.svelte.ts';
  import {
    recovery,
    retryOpeningLibrary,
    openAnotherLibrary,
    restoreLibraryBackup,
    startNewLibrary,
  } from '../stores/library-recovery.svelte.ts';

  let failure = $derived(library.openFailure);
</script>

{#if failure}
  <div class="ff-recovery" data-library-recovery data-failure={failure.kind}>
    <EmptyState title={failure.title} sub={failure.text}>
      {#if failure.canRetry}
        <Button label="Try again" variant="primary" size="small" disabled={recovery.busy} onclick={() => void retryOpeningLibrary()} />
      {/if}
      <Button label="Open another library" size="small" disabled={recovery.busy} onclick={() => void openAnotherLibrary()} />
      <Button label="Restore a backup" size="small" disabled={recovery.busy} onclick={() => void restoreLibraryBackup()} />
      <Button label="Start a new library" size="small" disabled={recovery.busy} onclick={() => void startNewLibrary()} />
    </EmptyState>
    {#if recovery.error}
      <p class="ff-recovery__error" role="alert" data-recovery-error>{recovery.error}</p>
    {/if}
  </div>
{/if}

<style>
  .ff-recovery {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--ff-space-6);
    width: 100%;
    height: 100%;
    padding: var(--ff-space-8);
    box-sizing: border-box;
  }
  .ff-recovery :global(.ff-empty-state) {
    width: 100%;
    max-width: 520px;
  }
  .ff-recovery :global(.ff-empty-state__actions) {
    flex-wrap: wrap;
    gap: var(--ff-space-4);
  }
  .ff-recovery__error {
    margin: 0;
    max-width: 520px;
    text-align: center;
    color: var(--ff-danger);
  }
</style>
