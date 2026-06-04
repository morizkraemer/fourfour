<script>
  // Sidebar section (Svelte port of sidebar-section.js). Group header + optional
  // '+' action, then its rows. Rows are slotted as children so the section never
  // hard-codes its contents.
  import './sidebar-section.css';
  import Icon from './Icon.svelte';

  let { label, showAdd = false, onAdd, onHeaderContextMenu, children } = $props();

  function add(e) {
    e.stopPropagation();
    onAdd?.();
  }
</script>

<div class="ff-sidebar-section">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="ff-sidebar-section__head"
    oncontextmenu={(e) => {
      e.preventDefault();
      onHeaderContextMenu?.(e);
    }}
  >
    <span class="ff-sidebar-section__label">{label}</span>
    {#if showAdd}
      <button class="ff-sidebar-section__add-btn" onclick={add}>
        <Icon name="plus" size={13} class="ff-sidebar-section__add-icon" />
      </button>
    {/if}
  </div>
  <div class="ff-sidebar-section__rows">{@render children?.()}</div>
</div>
