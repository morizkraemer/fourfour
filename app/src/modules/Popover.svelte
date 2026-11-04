<!--
  Popover.svelte — the card that opens under a trigger (the player's track BPM and key pencils).

  It is the `.ff-menu` surface with dialog padding and a small arrow pointing at the trigger. It is
  moved to <body>, stays inside the window, and closes on a click outside it (a click on the trigger
  is left to the trigger) or Esc.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import '$ds/components/menu.css';

  const GAP_PX = 10;
  const EDGE_PX = 8;
  const ARROW_EDGE_PX = 14;

  let {
    anchor,
    width,
    title,
    meta = '',
    label = title,
    onClose,
    children,
    foot,
    heading,
    dense = false,
  }: {
    anchor: HTMLElement | undefined;
    width: number;
    title: string;
    meta?: string;
    label?: string;
    onClose: () => void;
    children: Snippet;
    foot?: Snippet;
    heading?: Snippet;
    dense?: boolean;
  } = $props();

  let card: HTMLElement | undefined = $state();
  let left = $state(0);
  let top = $state(0);
  let arrowLeft = $state(0);

  function place() {
    if (!anchor || !card) return;
    const rect = anchor.getBoundingClientRect();
    const centre = rect.left + rect.width / 2;
    left = Math.max(EDGE_PX, Math.min(centre - width / 2, window.innerWidth - width - EDGE_PX));
    top = Math.max(EDGE_PX, Math.min(rect.bottom + GAP_PX, window.innerHeight - card.offsetHeight - EDGE_PX));
    arrowLeft = Math.max(ARROW_EDGE_PX, Math.min(centre - left, width - ARROW_EDGE_PX));
  }

  $effect(() => {
    place();
    const observer = new ResizeObserver(place);
    if (card) observer.observe(card);
    window.addEventListener('resize', place);
    return () => {
      observer.disconnect();
      window.removeEventListener('resize', place);
    };
  });

  $effect(() => {
    function onPointerDown(event: PointerEvent) {
      const target = event.target as Node;
      if (card?.contains(target) || anchor?.contains(target)) return;
      onClose();
    }
    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      event.stopPropagation();
      onClose();
    }
    document.addEventListener('pointerdown', onPointerDown, true);
    document.addEventListener('keydown', onKeyDown, true);
    return () => {
      document.removeEventListener('pointerdown', onPointerDown, true);
      document.removeEventListener('keydown', onKeyDown, true);
      if (anchor?.isConnected && (document.activeElement === document.body || card?.contains(document.activeElement))) anchor.focus();
    };
  });

  function toBody(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }
</script>

<div
  use:toBody
  bind:this={card}
  class="ff-menu ff-popover"
  class:ff-popover--dense={dense}
  role="dialog"
  aria-label={label}
  style:width="{width}px"
  style:left="{left}px"
  style:top="{top}px">
  <i class="ff-popover__arrow" style:left="{arrowLeft}px"></i>
  <div class="ff-popover__title">
    {#if heading}
      {@render heading()}
    {:else}
      <span class="ff-popover__name">{title}</span>
      {#if meta}<span class="ff-popover__meta">{meta}</span>{/if}
    {/if}
  </div>
  {@render children()}
  {#if foot}<div class="ff-popover__foot">{@render foot()}</div>{/if}
</div>

<style>
  .ff-popover {
    position: fixed;
    z-index: 10050;
    gap: var(--ff-space-5);
    padding: var(--ff-space-7);
    min-width: 0;
    border-radius: var(--ff-radius-xl);
  }
  .ff-popover--dense {
    gap: var(--ff-space-4);
    padding: var(--ff-space-5);
  }
  .ff-popover__arrow {
    position: absolute;
    top: -6px;
    width: 10px;
    height: 10px;
    margin-left: -5px;
    background: var(--ff-elev);
    border-left: 1px solid var(--ff-border-hi);
    border-top: 1px solid var(--ff-border-hi);
    transform: rotate(45deg);
  }
  .ff-popover__title {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--ff-space-4);
    height: 20px;
  }
  .ff-popover__name {
    font-size: var(--ff-type-h2);
    line-height: 20px;
    font-weight: 500;
    white-space: nowrap;
  }
  .ff-popover__meta {
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-small);
    color: var(--ff-muted);
    white-space: nowrap;
  }
  .ff-popover :global(.ff-popover__link) {
    padding: 0;
    border: none;
    background: none;
    color: var(--ff-muted);
    font: var(--ff-type-caption) var(--ff-font);
    text-decoration: underline;
    text-decoration-color: var(--ff-border-hi);
    text-underline-offset: 3px;
    white-space: nowrap;
    cursor: pointer;
  }
  .ff-popover :global(.ff-popover__link:hover:not(:disabled)) {
    color: var(--ff-text);
  }
  .ff-popover :global(.ff-popover__link:disabled) {
    opacity: 0.4;
    cursor: default;
  }
  .ff-popover__foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--ff-space-4);
    height: 28px;
  }
</style>
