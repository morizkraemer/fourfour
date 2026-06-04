<script>
  /**
   * Modal dialog — Svelte port of dialog.js (fixed overlay + backdrop).
   * Uses a positioned wrapper instead of native showModal() so the dialog still
   * opens after async work (e.g. post-import) in Tauri/WKWebView.
   */
  import { tick } from 'svelte';
  import './dialog.css';

  let {
    open = false,
    title = '',
    bodyText = '',
    variant = 'default',
    onClose,
    body,
    actions,
  } = $props();

  let panelEl = $state(null);
  let actionsEl = $state(null);

  $effect(() => {
    if (!open) return;

    function onKeydown(e) {
      if (e.key !== 'Escape' || e.defaultPrevented) return;
      const t = e.target;
      if (t instanceof Element && t.closest('.ff-settings-screen')) return;
      e.preventDefault();
      onClose?.();
    }

    window.addEventListener('keydown', onKeydown);
    void (async () => {
      await tick();
      const focusTarget =
        actionsEl?.querySelector<HTMLElement>('.ff-btn--primary') ??
        actionsEl?.querySelector<HTMLElement>('.ff-btn') ??
        panelEl;
      focusTarget?.focus();
    })();

    return () => window.removeEventListener('keydown', onKeydown);
  });

  function handleBackdropClick(e) {
    if (e.target === e.currentTarget) onClose?.();
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="ff-dialog-wrapper" role="presentation" onclick={handleBackdropClick}>
    <div class="ff-dialog-backdrop" aria-hidden="true"></div>
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      bind:this={panelEl}
      class="ff-dialog"
      class:ff-dialog--wide={variant === 'wide'}
      role="dialog"
      aria-modal="true"
      aria-labelledby={title ? 'ff-dialog-title' : undefined}
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
    >
      {#if title}
        <h2 id="ff-dialog-title" class="ff-dialog__title">{title}</h2>
      {/if}
      <div class="ff-dialog__body">
        {#if body}
          {@render body()}
        {:else if bodyText}
          {bodyText}
        {/if}
      </div>
      {#if actions}
        <div class="ff-dialog__actions" bind:this={actionsEl}>
          {@render actions()}
        </div>
      {/if}
    </div>
  </div>
{/if}
