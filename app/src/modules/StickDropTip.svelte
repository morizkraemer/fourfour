<script>
  import { dnd } from '../stores/dnd.svelte.ts';
  import { hoveredStickDropLabel } from '../stores/organize-drop.svelte.ts';

  const label = $derived(dnd.dropStickPath && dnd.dropStickSurface === 'row' ? hoveredStickDropLabel() : '');
</script>

{#if label}
  <div class="ff-stickdroptip" data-drop-label style:left="{dnd.pointerX + 14}px" style:top="{dnd.pointerY + 14}px">
    {label}
  </div>
{/if}

<style>
  .ff-stickdroptip {
    position: fixed;
    z-index: 1000;
    padding: 5px 11px;
    border-radius: 5px;
    background: var(--ff-accent);
    color: var(--ff-bg);
    font-size: var(--ff-type-compact);
    font-weight: 500;
    white-space: nowrap;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.5);
    pointer-events: none;
  }
</style>
