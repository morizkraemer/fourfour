<script>
  import { dnd } from '../stores/dnd.svelte.ts';
  import { dropFlash, cardKey, hoveredStickDropLabel } from '../stores/organize-drop.svelte.ts';
  import StickView from './StickView.svelte';

  let { volumePath } = $props();

  const dropHover = $derived(
    (dnd.kind === 'playlist' || dnd.kind === 'tracks') && dnd.dropStickPath === volumePath && dnd.dropStickSurface === 'board'
  );
  const flash = $derived(dropFlash.key === cardKey({ kind: 'stick', volumePath }) ? dropFlash.text : '');
</script>

<section
  class="ff-stickboard"
  class:ff-stickboard--drop={dropHover}
  data-stick-board
  data-drop-stick-path={volumePath}
  data-drop-surface="board"
  aria-label="On this stick"
>
  <StickView {volumePath} embedded />

  {#if dropHover}
    <div class="ff-stickboard__droplabel" data-drop-label>{hoveredStickDropLabel()}</div>
  {:else if flash}
    <div class="ff-stickboard__flash" role="status" data-stick-flash>{flash}</div>
  {/if}
</section>

<style>
  .ff-stickboard {
    position: relative;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .ff-stickboard__droplabel,
  .ff-stickboard__flash {
    position: absolute;
    z-index: 2;
    left: 50%;
    transform: translateX(-50%);
    padding: 5px 11px;
    border-radius: 5px;
    font-size: var(--ff-type-compact);
    font-weight: 500;
    white-space: nowrap;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.5);
    pointer-events: none;
  }
  .ff-stickboard__droplabel {
    top: 50%;
    transform: translate(-50%, -50%);
    background: var(--ff-accent);
    color: var(--ff-bg);
  }
  .ff-stickboard__flash {
    bottom: var(--ff-space-7);
    border: 1px solid var(--ff-border-hi);
    background: var(--ff-elev);
    color: var(--ff-text);
  }
  .ff-stickboard--drop {
    outline: 1px solid var(--ff-accent);
    outline-offset: -1px;
    background: var(--ff-hover);
  }
</style>
