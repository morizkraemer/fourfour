<script lang="ts">
  import { untrack } from 'svelte';
  import { keyColor } from '$ds';
  import Popover from './Popover.svelte';
  import { player, setTrackMode } from '../stores/player.svelte.ts';
  import { commitTrackKey, resetTrackFieldToAnalyzed } from '../stores/track-edits.svelte.ts';
  import { library } from '../stores/library.svelte.ts';
  import { camelotOfKey, keyCells, keyWrittenLike } from '../lib/tempo.ts';

  let { anchor }: { anchor: HTMLElement | undefined } = $props();

  const trackId = untrack(() => player.track?.id ?? null);
  const savedKey = untrack(() => player.track?.key ?? '');
  const current = camelotOfKey(savedKey);
  const currentCell = keyCells(current?.endsWith('A') ?? true).find((cell) => cell.camelot === current);
  const currentName = currentCell ? (current?.endsWith('A') ? `${currentCell.name.slice(0, -1)} minor` : `${currentCell.name} major`) : '';
  const majorCells = keyCells(false);
  const rows = keyCells(true).map((minor, index) => [minor, majorCells[index]]);

  let grid: HTMLElement | undefined = $state();
  let handSet = $derived(library.tracks.find((t) => t.id === trackId)?.keyLocked === true);

  $effect(() => {
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
    if (trackId !== null) void resetTrackFieldToAnalyzed(trackId, 'key');
    close();
  }

  function onGridKeyDown(event: KeyboardEvent) {
    const buttons = [...(grid?.querySelectorAll<HTMLElement>('[data-key]') ?? [])];
    const from = buttons.indexOf(document.activeElement as HTMLElement);
    if (from < 0) return;
    const row = Math.floor(from / 2);
    const column = from % 2;
    const to = {
      ArrowUp: Math.max(row - 1, 0) * 2 + column,
      ArrowDown: Math.min(row + 1, rows.length - 1) * 2 + column,
      ArrowLeft: row * 2,
      ArrowRight: row * 2 + 1,
    }[event.key];
    if (to === undefined) return;
    event.preventDefault();
    buttons[to]?.focus();
  }
</script>

<Popover {anchor} width={228} dense title="Track key" label="Track key" onClose={close}>
  {#snippet heading()}
    <span class="ff-keypop__now">
      {#if current}
        <b style:color={keyColor(current)}>{current}</b>
        <span>· {currentName}</span>
      {:else}
        <span>Track key</span>
      {/if}
    </span>
    <button type="button" class="ff-popover__link" disabled={!handSet} onclick={reset}>Reset to analyzed</button>
  {/snippet}
  <div class="ff-keypop__heads" aria-hidden="true"><span>Minor</span><span>Major</span></div>
  <div class="ff-keypop__grid" role="toolbar" tabindex="-1" aria-label="Keys" bind:this={grid} onkeydown={onGridKeyDown}>
    {#each rows as row (row[0].camelot)}
      {#each row as cell (cell.camelot)}
        <button
          type="button"
          class="ff-keypop__key"
          class:ff-keypop__key--on={cell.camelot === current}
          style:--kc={keyColor(cell.camelot)}
          data-key={cell.camelot}
          data-current={cell.camelot === current}
          aria-label={`${cell.camelot}, ${cell.name}`}
          onclick={() => pick(cell.camelot)}><b>{cell.camelot}</b><span>{cell.name}</span></button>
      {/each}
    {/each}
  </div>
</Popover>

<style>
  .ff-keypop__now {
    display: flex;
    align-items: baseline;
    gap: var(--ff-space-2);
    font-size: var(--ff-type-row);
  }
  .ff-keypop__now b {
    font: 600 var(--ff-type-compact) var(--ff-font-mono);
  }
  .ff-keypop__heads,
  .ff-keypop__grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    column-gap: var(--ff-space-3);
    margin: 0 calc(-1 * var(--ff-space-2));
  }
  .ff-keypop__heads {
    margin-bottom: calc(-1 * var(--ff-space-2));
    font-size: var(--ff-type-label);
    color: var(--ff-muted);
  }
  .ff-keypop__heads span {
    padding: 0 var(--ff-space-3);
  }
  .ff-keypop__key {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 18px;
    padding: 0 var(--ff-space-3);
    border: none;
    border-radius: var(--ff-radius-sm);
    background: none;
    text-align: left;
    cursor: pointer;
    outline: none;
  }
  .ff-keypop__key b {
    width: 26px;
    font: 600 var(--ff-type-caption) var(--ff-font-mono);
    color: var(--kc);
  }
  .ff-keypop__key span {
    font: var(--ff-type-caption) var(--ff-font);
    color: var(--ff-text-mid);
  }
  .ff-keypop__key:hover,
  .ff-keypop__key:focus-visible {
    background: var(--ff-hover);
  }
  .ff-keypop__key--on,
  .ff-keypop__key--on:hover,
  .ff-keypop__key--on:focus-visible {
    background: color-mix(in srgb, var(--kc) 18%, transparent);
  }
  .ff-keypop__key--on span {
    color: var(--ff-text);
  }
</style>
