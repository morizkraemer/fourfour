<script>
  /**
   * Preferences sheet: a centred modal with a page list on the left (label plus live status)
   * and one short page on the right. Esc and a click on the scrim close it.
   */
  import { tick } from 'svelte';
  import './settings-sheet.css';
  import Icon from './Icon.svelte';
  import Kbd from './Kbd.svelte';

  /** @typedef {{ key: string, label: string, status?: string }} SheetPage */

  /** @type {{ open?: boolean, pages?: SheetPage[], activeKey?: string, shortcutHint?: string, onSelect?: (key: string) => void, onClose?: () => void, children?: import('svelte').Snippet }} */
  let { open = false, pages = [], activeKey = '', shortcutHint = '', onSelect, onClose, children } = $props();

  let sheetEl = $state(null);
  let returnFocusTo = null;
  let title = $derived(pages.find((p) => p.key === activeKey)?.label ?? '');

  $effect(() => {
    if (!open) return;
    returnFocusTo = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    void tick().then(() => sheetEl?.focus());
    return () => {
      returnFocusTo?.focus?.();
      returnFocusTo = null;
    };
  });

  function onKeydown(e) {
    if (e.key === 'Escape' && !e.defaultPrevented) {
      e.preventDefault();
      e.stopPropagation();
      onClose?.();
    }
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="ff-settings-scrim ff-settings-screen" onclick={(e) => e.target === e.currentTarget && onClose?.()}>
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      bind:this={sheetEl}
      class="ff-settings-sheet"
      role="dialog"
      aria-modal="true"
      aria-label="Preferences"
      tabindex="-1"
      onkeydown={onKeydown}
    >
      <div class="ff-settings-sheet__top">
        <span class="ff-settings-sheet__title">Preferences</span>
        {#if shortcutHint}<Kbd key={shortcutHint} />{/if}
        <button type="button" class="ff-settings-sheet__close" aria-label="Close" onclick={onClose}>
          <Icon name="x" size={14} />
        </button>
      </div>
      <div class="ff-settings-sheet__body">
        <nav class="ff-settings-sheet__nav" aria-label="Preferences pages">
          {#each pages as page (page.key)}
            <button
              type="button"
              class="ff-settings-sheet__nav-item"
              class:ff-settings-sheet__nav-item--on={page.key === activeKey}
              data-page={page.key}
              onclick={() => onSelect?.(page.key)}
            >
              <span class="ff-settings-sheet__nav-label">{page.label}</span>
              {#if page.status}<em class="ff-settings-sheet__nav-status">{page.status}</em>{/if}
            </button>
          {/each}
        </nav>
        <div class="ff-settings-sheet__page" data-page-body={activeKey}>
          <h2 class="ff-settings-sheet__heading">{title}</h2>
          {@render children?.()}
        </div>
      </div>
    </div>
  </div>
{/if}
