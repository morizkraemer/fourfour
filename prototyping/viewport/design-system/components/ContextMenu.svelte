<script>
  import './menu.css';
  import './checkbox.css';

  const CHECK_SVG =
    '<svg width="10" height="10" viewBox="0 0 10 10" fill="none" xmlns="http://www.w3.org/2000/svg"><polyline points="2,5.5 4.2,7.5 8,2.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>';

  /**
   * @typedef {object} MenuItem
   * @property {string} [label]
   * @property {string} [shortcut]
   * @property {'default'|'danger'|'disabled'} [variant]
   * @property {boolean} [disabled]
   * @property {boolean} [checked]
   * @property {boolean} [keepOpen]
   * @property {boolean} [isSeparator]
   * @property {MenuItem[]} [items]
   * @property {() => void} [onClick]
   */

  /** @type {{ open?: boolean, x?: number, y?: number, items?: MenuItem[], onClose?: () => void }} */
  let { open = false, x = 0, y = 0, items = [], onClose } = $props();

  let menuEl = $state(null);
  let openSubmenuIndex = $state(null);

  $effect(() => {
    if (!open) {
      openSubmenuIndex = null;
      return;
    }
    function onDoc(e) {
      if (menuEl && !menuEl.contains(e.target)) onClose?.();
    }
    function onKey(e) {
      if (e.key === 'Escape') onClose?.();
    }
    function onWindowBlur() {
      onClose?.();
    }
    function onVisibilityChange() {
      if (document.visibilityState === 'hidden') onClose?.();
    }
    const t = setTimeout(() => {
      document.addEventListener('pointerdown', onDoc, true);
      document.addEventListener('keydown', onKey);
      window.addEventListener('blur', onWindowBlur);
      document.addEventListener('visibilitychange', onVisibilityChange);
    }, 0);
    return () => {
      clearTimeout(t);
      document.removeEventListener('pointerdown', onDoc, true);
      document.removeEventListener('keydown', onKey);
      window.removeEventListener('blur', onWindowBlur);
      document.removeEventListener('visibilitychange', onVisibilityChange);
    };
  });

  function clampMenu(node) {
    const rect = node.getBoundingClientRect();
    let left = x;
    let top = y;
    if (left + rect.width > window.innerWidth - 8) {
      left = Math.max(8, window.innerWidth - rect.width - 8);
    }
    if (top + rect.height > window.innerHeight - 8) {
      top = Math.max(8, window.innerHeight - rect.height - 8);
    }
    node.style.left = `${left}px`;
    node.style.top = `${top}px`;
  }

  /** Flip flyout left when it would overflow the viewport. */
  function clampSubmenu(node) {
    const rect = node.getBoundingClientRect();
    if (rect.right > window.innerWidth - 8) {
      node.classList.add('ff-menu--submenu-left');
    }
    if (rect.bottom > window.innerHeight - 8) {
      const shift = rect.bottom - window.innerHeight + 8;
      node.style.top = `${-4 - shift}px`;
    }
  }

  /** @param {MenuItem} item */
  function runSubItem(item) {
    if (item.disabled) return;
    item.onClick?.();
    onClose?.();
  }

  /** @param {MenuItem} item */
  function runItem(item) {
    if (item.disabled || item.items?.length) return;
    item.onClick?.();
    if (!item.keepOpen) onClose?.();
  }

  /** @param {number} index */
  function openSubmenu(index) {
    openSubmenuIndex = index;
  }

  /** @param {number} index */
  function closeSubmenu(index) {
    if (openSubmenuIndex === index) openSubmenuIndex = null;
  }
</script>

{#snippet menuItems(list, depth = 0)}
  {#each list as item, i}
    {#if item.isSeparator}
      <div class="ff-menu__separator"></div>
    {:else}
      {@const variant = item.disabled ? 'disabled' : (item.variant || 'default')}
      {@const hasSub = (item.items?.length ?? 0) > 0}
      {@const subOpen = openSubmenuIndex === i && depth === 0}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="ff-menu__item ff-menu__item--{variant}"
        class:ff-menu__item--toggle={item.checked !== undefined}
        class:ff-menu__item--submenu={hasSub}
        class:ff-menu__item--submenu-open={subOpen}
        onmouseenter={() => hasSub && depth === 0 && openSubmenu(i)}
        onmouseleave={() => hasSub && depth === 0 && closeSubmenu(i)}
        onclick={() => {
          if (hasSub) return;
          runItem(item);
        }}
      >
        <span class="ff-menu__item-label">
          {#if item.checked !== undefined}
            <span
              class="ff-checkbox ff-menu__checkbox"
              class:ff-checkbox--on={item.checked}
              aria-hidden="true"
            >
              <span class="ff-checkbox__box">
                <span class="ff-checkbox__icon">{@html CHECK_SVG}</span>
              </span>
              <span class="ff-checkbox__label">{item.label ?? ''}</span>
            </span>
          {:else}
            {item.label ?? ''}
          {/if}
        </span>
        <span class="ff-menu__item-trail">
          {#if item.shortcut}
            <kbd class="ff-menu__item-shortcut">{item.shortcut}</kbd>
          {/if}
          {#if hasSub}
            <span class="ff-menu__submenu-chevron" aria-hidden="true">›</span>
          {/if}
        </span>
        {#if hasSub && subOpen}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="ff-menu ff-menu--submenu"
            use:clampSubmenu
            onmouseenter={() => openSubmenu(i)}
            onclick={(e) => e.stopPropagation()}
          >
            {#each item.items as subItem}
              {#if subItem.isSeparator}
                <div class="ff-menu__separator"></div>
              {:else}
                {@const subVariant = subItem.disabled ? 'disabled' : (subItem.variant || 'default')}
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div
                  class="ff-menu__item ff-menu__item--{subVariant}"
                  onclick={() => runSubItem(subItem)}
                >
                  <span class="ff-menu__item-label">
                    {#if subItem.checked !== undefined}
                      <span
                        class="ff-checkbox ff-menu__checkbox"
                        class:ff-checkbox--on={subItem.checked}
                        aria-hidden="true"
                      >
                        <span class="ff-checkbox__box">
                          <span class="ff-checkbox__icon">{@html CHECK_SVG}</span>
                        </span>
                        <span class="ff-checkbox__label">{subItem.label ?? ''}</span>
                      </span>
                    {:else}
                      {subItem.label ?? ''}
                    {/if}
                  </span>
                  {#if subItem.shortcut}
                    <span class="ff-menu__item-trail">
                      <kbd class="ff-menu__item-shortcut">{subItem.shortcut}</kbd>
                    </span>
                  {/if}
                </div>
              {/if}
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  {/each}
{/snippet}

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="ff-menu ff-context-menu"
    bind:this={menuEl}
    use:clampMenu
    style="position: fixed; left: {x}px; top: {y}px; z-index: 10000;"
    onclick={(e) => e.stopPropagation()}
  >
    {@render menuItems(items)}
  </div>
{/if}

<style>
  .ff-menu__checkbox {
    pointer-events: none;
  }

  .ff-menu__item--toggle .ff-checkbox__label {
    color: inherit;
  }

  .ff-menu__item-trail {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .ff-menu__submenu-chevron {
    font-size: 11px;
    line-height: 1;
    color: var(--ff-muted);
    margin-left: 2px;
  }

  .ff-menu__item--submenu-open .ff-menu__submenu-chevron,
  .ff-menu__item--submenu:hover .ff-menu__submenu-chevron {
    color: var(--ff-text-mid);
  }

  .ff-menu__item--submenu {
    position: relative;
  }

  .ff-menu--submenu {
    position: absolute;
    left: calc(100% + 4px);
    top: -4px;
    min-width: 168px;
    z-index: 1;
  }

  .ff-menu--submenu.ff-menu--submenu-left {
    left: auto;
    right: calc(100% + 4px);
  }
</style>
