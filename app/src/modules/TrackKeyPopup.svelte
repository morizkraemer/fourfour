<!--
  TrackKeyPopup.svelte — the player's track key pencil: a Minor / Major tab and the 12 keys of that
  mode. The current key is filled, the keys that mix with it are tinted. Arrow keys move over the
  grid; clicking a key or Enter saves it and closes.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import { SegmentedControl, keyColor } from '$ds';
  import Popover from './Popover.svelte';
  import { player, setTrackMode } from '../stores/player.svelte.ts';
  import { commitTrackKey, resetTrackToAnalyzed } from '../stores/track-edits.svelte.ts';
  import { library } from '../stores/library.svelte.ts';
  import { camelotOfKey, harmonicNeighbours, keyCells, keyWrittenLike } from '../lib/tempo.ts';

  const COLUMNS = 3;

  let { anchor }: { anchor: HTMLElement | undefined } = $props();

  const trackId = untrack(() => player.track?.id ?? null);
  const savedKey = untrack(() => player.track?.key ?? '');
  const current = camelotOfKey(savedKey);
  const neighbours = current ? harmonicNeighbours(current) : [];
  const currentCell = keyCells(current?.endsWith('A') ?? true).find((cell) => cell.camelot === current);

  let minor = $state(current?.endsWith('A') ?? true);
  let cells = $derived(keyCells(minor));
  let grid: HTMLElement | undefined = $state();
  let handSet = $derived(library.tracks.find((t) => t.id === trackId)?.keyLocked === true);

  $effect(() => {
    if (!cells.length) return;
    untrack(() => {
      const target = grid?.querySelector<HTMLElement>('[data-current="true"]') ?? grid?.querySelector<HTMLElement>('[data-key]');
      target?.focus();
    });
  });

  $effect(() => {
    if (player.track?.id !== trackId) close();
  });

  function close() {
    setTrackMode('key', false);
  }

  function pick(camelot: string) {
    if (camelot !== current && trackId !== null) void commitTrackKey(trackId, keyWrittenLike(camelot, savedKey));
    close();
  }

  function reset() {
    if (trackId !== null) void resetTrackToAnalyzed(trackId);
    close();
  }

  function onGridKeyDown(event: KeyboardEvent) {
    const move = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -COLUMNS, ArrowDown: COLUMNS }[event.key];
    if (move === undefined) return;
    event.preventDefault();
    const buttons = [...(grid?.querySelectorAll<HTMLElement>('[data-key]') ?? [])];
    const from = buttons.indexOf(document.activeElement as HTMLElement);
    const to = Math.max(0, Math.min(buttons.length - 1, (from < 0 ? 0 : from) + move));
    buttons[to]?.focus();
  }
</script>

<Popover {anchor} width={280} title="Track key" meta={currentCell ? `${currentCell.camelot} · ${currentCell.name}` : ''} label="Track key" onClose={close}>
  <SegmentedControl items={['Minor', 'Major']} activeIndex={minor ? 0 : 1} onChange={(index: number) => (minor = index === 0)} />
  <div class="ff-keypop__grid" role="toolbar" tabindex="-1" aria-label={minor ? 'Minor keys' : 'Major keys'} bind:this={grid} onkeydown={onGridKeyDown}>
    {#each cells as cell (cell.camelot)}
      <button
        type="button"
        class="ff-keypop__key"
        class:ff-keypop__key--on={cell.camelot === current}
        class:ff-keypop__key--fit={neighbours.includes(cell.camelot)}
        style:--kc={keyColor(cell.camelot)}
        data-key={cell.camelot}
        data-current={cell.camelot === current}
        aria-label={`${cell.camelot}, ${cell.name}`}
        onclick={() => pick(cell.camelot)}><b>{cell.camelot}</b><span>· {cell.name}</span></button>
    {/each}
  </div>
  <div class="ff-keypop__foot">
    <span class="ff-keypop__legend">
      <span><i class="ff-keypop__swatch ff-keypop__swatch--on"></i>current</span>
      <span><i class="ff-keypop__swatch ff-keypop__swatch--fit"></i>mixes well</span>
    </span>
    <button type="button" class="ff-popover__link" disabled={!handSet} onclick={reset}>Reset to analyzed</button>
  </div>
</Popover>

<style>
  .ff-keypop__grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--ff-space-2);
  }
  .ff-keypop__key {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--ff-space-2);
    height: 28px;
    padding: 0;
    border: 1px solid transparent;
    border-radius: var(--ff-radius-md);
    background: transparent;
    color: var(--kc);
    cursor: pointer;
    outline: none;
  }
  .ff-keypop__key b {
    font: 600 var(--ff-type-num) var(--ff-font-mono);
  }
  .ff-keypop__key span {
    font: var(--ff-type-small) var(--ff-font);
    color: var(--ff-text-mid);
  }
  .ff-keypop__key:hover,
  .ff-keypop__key:focus-visible {
    background: var(--ff-select);
    border-color: var(--ff-text-mid);
  }
  .ff-keypop__key--fit {
    background: color-mix(in srgb, var(--kc) 16%, transparent);
    border-color: color-mix(in srgb, var(--kc) 60%, transparent);
  }
  .ff-keypop__key--on,
  .ff-keypop__key--on:hover,
  .ff-keypop__key--on:focus-visible {
    background: var(--kc);
    border-color: var(--kc);
    color: var(--ff-bg);
  }
  .ff-keypop__key--on span {
    color: color-mix(in srgb, var(--ff-bg) 70%, transparent);
  }
  .ff-keypop__key--on:focus-visible {
    box-shadow: 0 0 0 2px var(--ff-text-mid);
  }
  .ff-keypop__foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--ff-space-4);
    height: 16px;
  }
  .ff-keypop__legend {
    display: inline-flex;
    gap: var(--ff-space-5);
    font-size: var(--ff-type-small);
    color: var(--ff-muted);
  }
  .ff-keypop__legend > span {
    display: inline-flex;
    align-items: center;
    gap: var(--ff-space-2);
  }
  .ff-keypop__swatch {
    width: 10px;
    height: 10px;
    border-radius: var(--ff-radius-xs);
  }
  .ff-keypop__swatch--on {
    background: var(--ff-text-mid);
  }
  .ff-keypop__swatch--fit {
    border: 1px solid var(--ff-text-mid);
    background: color-mix(in srgb, var(--ff-text-mid) 16%, transparent);
  }
</style>
