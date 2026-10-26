<script>
  /**
   * Command palette overlay (Svelte port of command-palette.js).
   * Presentational: the host supplies grouped, already-ranked items and runs the chosen one.
   * Keyboard: ↑/↓ and Tab/Shift+Tab move, Enter runs, Esc closes.
   */
  import './command-palette.css';
  import { tick } from 'svelte';
  import Icon from './Icon.svelte';

  /**
   * @typedef {{ id: string, name: string, sub?: string, right?: string, icon?: string, ranges?: number[] }} PaletteItem
   * @typedef {{ key: string, title: string, items: PaletteItem[] }} PaletteGroup
   */

  /** @type {{ open?: boolean, query?: string, groups?: PaletteGroup[], placeholder?: string, emptyLabel?: string, onRun?: (item: PaletteItem, group: PaletteGroup) => void, onClose?: () => void }} */
  let {
    open = false,
    query = $bindable(''),
    groups = [],
    placeholder = 'Type a command, playlist or track…',
    emptyLabel = 'No matches',
    onRun,
    onClose,
  } = $props();

  let inputEl = $state(null);
  let listEl = $state(null);
  let activeIndex = $state(0);
  let returnFocusTo = null;

  let flat = $derived(groups.flatMap((g) => g.items.map((item) => ({ item, group: g }))));

  $effect(() => {
    query;
    activeIndex = 0;
  });

  $effect(() => {
    if (activeIndex >= flat.length) activeIndex = Math.max(0, flat.length - 1);
  });

  $effect(() => {
    if (!open) return;
    returnFocusTo = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    void tick().then(() => inputEl?.focus());
    return () => {
      returnFocusTo?.focus?.();
      returnFocusTo = null;
    };
  });

  $effect(() => {
    activeIndex;
    if (!open || !listEl) return;
    listEl.querySelector('[data-active="true"]')?.scrollIntoView({ block: 'nearest' });
  });

  function move(step) {
    if (flat.length === 0) return;
    activeIndex = (activeIndex + step + flat.length) % flat.length;
  }

  function run(entry) {
    if (entry) onRun?.(entry.item, entry.group);
  }

  function onKeydown(e) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onClose?.();
    } else if (e.key === 'ArrowDown' || (e.key === 'Tab' && !e.shiftKey)) {
      e.preventDefault();
      move(1);
    } else if (e.key === 'ArrowUp' || (e.key === 'Tab' && e.shiftKey)) {
      e.preventDefault();
      move(-1);
    } else if (e.key === 'Enter' && !e.isComposing) {
      e.preventDefault();
      run(flat[activeIndex]);
    }
  }

  function segments(name, ranges) {
    if (!ranges || ranges.length === 0) return [{ text: name, hit: false }];
    const hit = new Set(ranges);
    const out = [];
    for (let i = 0; i < name.length; i++) {
      const isHit = hit.has(i);
      const last = out[out.length - 1];
      if (last && last.hit === isHit) last.text += name[i];
      else out.push({ text: name[i], hit: isHit });
    }
    return out;
  }

  function flatIndex(group, itemIndex) {
    let n = 0;
    for (const g of groups) {
      if (g === group) return n + itemIndex;
      n += g.items.length;
    }
    return -1;
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="ff-cp-overlay" onclick={(e) => e.target === e.currentTarget && onClose?.()}>
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="ff-cp" role="dialog" aria-modal="true" aria-label="Command palette" tabindex="-1" onkeydown={onKeydown}>
      <div class="ff-cp__input-row">
        <Icon name="search" size={14} class="ff-cp__search-icon" />
        <input
          bind:this={inputEl}
          bind:value={query}
          class="ff-cp__input"
          type="text"
          role="combobox"
          aria-expanded="true"
          aria-controls="ff-cp-list"
          aria-activedescendant={flat.length ? `ff-cp-item-${activeIndex}` : undefined}
          autocomplete="off"
          spellcheck="false"
          {placeholder}
        />
        <kbd class="ff-cp__esc-kbd">esc</kbd>
      </div>

      <div class="ff-cp__content" id="ff-cp-list" role="listbox" bind:this={listEl}>
        {#each groups as group (group.key)}
          {#if group.items.length}
            <div class="ff-cp__section-head">{group.title}</div>
            {#each group.items as item, i (item.id)}
              {@const index = flatIndex(group, i)}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <div
                id="ff-cp-item-{index}"
                class="ff-cp__row"
                class:ff-cp__row--active={index === activeIndex}
                role="option"
                aria-selected={index === activeIndex}
                data-active={index === activeIndex}
                tabindex="-1"
                onmousemove={() => (activeIndex = index)}
                onclick={() => run({ item, group })}
              >
                <Icon name={item.icon ?? 'chevron-right'} size={14} class="ff-cp__row-icon" />
                <span class="ff-cp__row-name">
                  {#each segments(item.name, item.ranges) as seg}{#if seg.hit}<b class="ff-cp__hit">{seg.text}</b>{:else}{seg.text}{/if}{/each}{#if item.sub}<span class="ff-cp__row-sub"> {item.sub}</span>{/if}
                </span>
                {#if item.right}<span class="ff-cp__row-right">{item.right}</span>{/if}
              </div>
            {/each}
          {/if}
        {/each}
        {#if flat.length === 0}
          <div class="ff-cp__empty">{emptyLabel}</div>
        {/if}
      </div>

      <div class="ff-cp__footer">
        <span class="ff-cp__footer-left">↑↓ navigate  ⏎ run  esc close</span>
        <span class="ff-cp__footer-right">{flat.length} {flat.length === 1 ? 'result' : 'results'}</span>
      </div>
    </div>
  </div>
{/if}
