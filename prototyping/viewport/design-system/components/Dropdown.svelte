<script>
  /**
   * Select control — input-styled trigger + menu popup (§2.3 trigger, §2.16 menu).
   * Not a native <select>; the open list uses .ff-menu styling.
   */
  import './dropdown.css';
  import './menu.css';
  import Icon from './Icon.svelte';

  /**
   * @typedef {{ value: string, label: string }} DropdownOption
   */

  let {
    options = [],
    value = $bindable(''),
    disabled = false,
    onChange,
  } = $props();

  let open = $state(false);
  let rootEl = $state(null);
  let triggerEl = $state(null);
  let menuEl = $state(null);
  let menuLeft = $state(0);
  let menuTop = $state(0);
  let menuMinWidth = $state(160);

  let displayLabel = $derived(
    options.find((o) => o.value === value)?.label ?? value,
  );

  function close() {
    open = false;
  }

  function toggle() {
    if (disabled || options.length === 0) return;
    open = !open;
  }

  /** @param {string} next */
  function pick(next) {
    if (next === value) {
      close();
      return;
    }
    value = next;
    onChange?.(next);
    close();
  }

  function positionMenu() {
    if (!triggerEl || !menuEl) return;
    const tr = triggerEl.getBoundingClientRect();
    menuMinWidth = Math.max(180, Math.round(tr.width));
    let left = tr.left;
    let top = tr.bottom + 4;
    const rect = menuEl.getBoundingClientRect();
    if (left + rect.width > window.innerWidth - 8) {
      left = Math.max(8, window.innerWidth - rect.width - 8);
    }
    if (top + rect.height > window.innerHeight - 8) {
      top = Math.max(8, tr.top - rect.height - 4);
    }
    menuLeft = left;
    menuTop = top;
  }

  $effect(() => {
    if (!open) return;
    requestAnimationFrame(() => positionMenu());
    function onDoc(e) {
      const t = e.target;
      if (rootEl?.contains(t) || menuEl?.contains(t)) return;
      close();
    }
    function onKey(e) {
      if (e.key === 'Escape') {
        e.stopPropagation();
        close();
      }
    }
    const t = setTimeout(() => {
      document.addEventListener('mousedown', onDoc);
      document.addEventListener('keydown', onKey);
    }, 0);
    return () => {
      clearTimeout(t);
      document.removeEventListener('mousedown', onDoc);
      document.removeEventListener('keydown', onKey);
    };
  });
</script>

<div class="ff-dropdown" class:ff-dropdown--open={open} bind:this={rootEl}>
  <button
    type="button"
    class="ff-dropdown__trigger"
    bind:this={triggerEl}
    aria-haspopup="listbox"
    aria-expanded={open}
    {disabled}
    onclick={toggle}
  >
    <span class="ff-dropdown__value">{displayLabel}</span>
    <span class="ff-dropdown__chevron" aria-hidden="true">
      <Icon name="chevron-down" size={12} />
    </span>
  </button>

  {#if open}
    <div
      class="ff-dropdown__menu ff-menu"
      bind:this={menuEl}
      role="listbox"
      style:left="{menuLeft}px"
      style:top="{menuTop}px"
      style:min-width="{menuMinWidth}px"
      onclick={(e) => e.stopPropagation()}
    >
      {#each options as opt (opt.value)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          role="option"
          aria-selected={opt.value === value}
          class="ff-menu__item ff-menu__item--default"
          onclick={() => pick(opt.value)}
        >
          <span class="ff-menu__item-label">
            <span class="ff-dropdown__check">{opt.value === value ? '✓' : ' '}</span>
            {opt.label}
          </span>
        </div>
      {/each}
    </div>
  {/if}
</div>
