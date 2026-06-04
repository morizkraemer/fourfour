<script>
  import './drag-ghost.css';

  let { label = '', count = 1, x = 0, y = 0, visible = false } = $props();
</script>

{#if visible}
  <div
    class="ff-drag-ghost ff-drag-ghost--floating"
    style="left: {x + 12}px; top: {y + 12}px;"
  >
    {#if count > 1}
      <span class="ff-drag-ghost__badge">{count}</span>
      <span class="ff-drag-ghost__label">{count} tracks</span>
    {:else}
      <span class="ff-drag-ghost__label">{label || '1 track'}</span>
    {/if}
  </div>
{/if}

<style>
  .ff-drag-ghost--floating {
    position: fixed;
    z-index: 10001;
    pointer-events: none;
  }
</style>
