<script>
  // Status bar (Svelte port of status-bar.js). Left: optional spinner + message
  // (· context). Right: optional thin progress bar, count, and a `rightExtras`
  // snippet (the v2 statusbar slots a level meter + settings button here).
  import './status-bar.css';
  import Spinner from './Spinner.svelte';
  import ProgressBar from './ProgressBar.svelte';

  let {
    message,
    context,
    count,
    showSpinner = false,
    progress,
    rightExtras,
  } = $props();

  let hasProgress = $derived(progress !== undefined && progress !== null);
  let hasCount = $derived(count !== undefined && count !== null);
</script>

<div class="ff-status-bar">
  <div class="ff-status-bar__left">
    {#if showSpinner}
      <Spinner size="small" class="ff-status-bar__spinner" />
    {/if}
    <span class="ff-status-bar__message">{message}</span>
    {#if context}
      <span class="ff-status-bar__separator">·</span>
      <span class="ff-status-bar__context">{context}</span>
    {/if}
  </div>
  <div class="ff-status-bar__right">
    {#if hasProgress}
      <div class="ff-status-bar__progress-wrapper">
        <ProgressBar value={progress} variant="thin" />
      </div>
    {/if}
    {#if hasCount}
      <span class="ff-status-bar__count">{count}</span>
    {/if}
    {@render rightExtras?.()}
  </div>
</div>
