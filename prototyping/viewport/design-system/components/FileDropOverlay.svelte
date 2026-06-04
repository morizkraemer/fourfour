<script>
  import './drag-ghost.css';

  let { visible = false, title = 'Drop audio files here', sub = '' } = $props();
</script>

{#if visible}
  <div class="ff-file-drop-scrim">
    <div class="ff-drop-overlay">
      <div class="ff-drop-overlay__title">{title}</div>
      {#if sub}
        <div class="ff-drop-overlay__sub">{sub}</div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .ff-file-drop-scrim {
    position: fixed;
    inset: 0;
    z-index: 9998;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.45);
    pointer-events: none;
  }
</style>
