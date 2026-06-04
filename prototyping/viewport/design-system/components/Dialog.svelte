<script>
  /**
   * Modal dialog — Svelte port of dialog.js + native <dialog> for Tauri/WKWebView.
   * Pass copy via `bodyText` or a `body` snippet; actions via an `actions` snippet.
   */
  import './dialog.css';
  import './dialog-native.css';

  let {
    open = false,
    title = '',
    bodyText = '',
    variant = 'default',
    onClose,
    body,
    actions,
  } = $props();

  let dialogEl;

  $effect(() => {
    if (!dialogEl) return;
    if (open) {
      if (!dialogEl.open) dialogEl.showModal();
    } else if (dialogEl.open) {
      dialogEl.close();
    }
  });

  function handleBackdropClick(e) {
    if (e.target === dialogEl) onClose?.();
  }

  function handleCancel(e) {
    e.preventDefault();
    onClose?.();
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<dialog
  bind:this={dialogEl}
  class="ff-dialog-native"
  oncancel={handleCancel}
  onclick={handleBackdropClick}
>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="ff-dialog"
    class:ff-dialog--wide={variant === 'wide'}
    onclick={(e) => e.stopPropagation()}
  >
    {#if title}
      <h2 class="ff-dialog__title">{title}</h2>
    {/if}
    <div class="ff-dialog__body">
      {#if body}
        {@render body()}
      {:else if bodyText}
        {bodyText}
      {/if}
    </div>
    {#if actions}
      <div class="ff-dialog__actions">
        {@render actions()}
      </div>
    {/if}
  </div>
</dialog>
