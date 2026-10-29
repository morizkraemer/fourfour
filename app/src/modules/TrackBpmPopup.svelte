<!--
  TrackBpmPopup.svelte — the player's track BPM pencil: type the BPM, halve or double it, or tap it in.
  Save is enabled once the value differs; Enter saves, Esc cancels. Saving re-spaces the beat grid
  from its first downbeat (commitTrackBpm).
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import '$ds/components/button.css';
  import Popover from './Popover.svelte';
  import { player, setTrackMode } from '../stores/player.svelte.ts';
  import { commitTrackBpm, resetTrackToAnalyzed } from '../stores/track-edits.svelte.ts';
  import { library } from '../stores/library.svelte.ts';
  import { doubleBpm, halveBpm, parseBpmEntry } from '../lib/tempo.ts';
  import { createTapTempo } from '../lib/tapTempo.ts';

  let { anchor }: { anchor: HTMLElement | undefined } = $props();

  const trackId = untrack(() => player.track?.id ?? null);
  const savedBpm = untrack(() => Number(player.track?.bpm));
  let draft = $state(savedBpm.toFixed(2));
  let inputEl: HTMLInputElement | undefined = $state();
  const tapTempo = createTapTempo();

  let typed = $derived(parseBpmEntry(draft));
  let changed = $derived(typed !== null && typed !== Math.round(savedBpm * 100) / 100);
  let handSet = $derived(library.tracks.find((t) => t.id === trackId)?.bpmLocked === true);

  $effect(() => {
    inputEl?.focus();
    inputEl?.select();
  });

  $effect(() => {
    if (player.track?.id !== trackId) close();
  });

  function close() {
    setTrackMode('bpm', false);
  }

  function save() {
    if (!changed || typed === null || trackId === null) return;
    if (commitTrackBpm(trackId, typed)) close();
  }

  function scale(next: (bpm: number) => number | null) {
    const scaled = next(typed ?? savedBpm);
    if (scaled !== null) draft = scaled.toFixed(2);
    inputEl?.focus();
  }

  function tap() {
    const bpm = tapTempo.tap();
    if (bpm !== null) draft = bpm.toFixed(2);
  }

  function reset() {
    if (trackId !== null) void resetTrackToAnalyzed(trackId);
    close();
  }

  function onInputKeyDown(event: KeyboardEvent) {
    if (event.key !== 'Enter') return;
    event.preventDefault();
    save();
  }
</script>

<Popover {anchor} width={264} title="Track BPM" meta={handSet ? 'hand-set' : 'analyzed'} label="Track BPM" onClose={close}>
  <div class="ff-bpmpop__row">
    <input
      bind:this={inputEl}
      bind:value={draft}
      class="ff-bpmpop__input"
      class:ff-bpmpop__input--invalid={typed === null}
      type="text"
      inputmode="decimal"
      spellcheck="false"
      autocomplete="off"
      aria-label="Track BPM"
      onkeydown={onInputKeyDown} />
    <div class="ff-bpmpop__scale">
      <button type="button" title="Halve the track BPM" aria-label="Halve track BPM" onclick={() => scale(halveBpm)}>÷2</button>
      <button type="button" title="Double the track BPM" aria-label="Double track BPM" onclick={() => scale(doubleBpm)}>×2</button>
    </div>
    <button type="button" class="ff-btn ff-btn--default" title="Tap the beat four times or more" onclick={tap}>Tap</button>
  </div>
  {#snippet foot()}
    <button type="button" class="ff-popover__link" disabled={!handSet} onclick={reset}>Reset to analyzed</button>
    <button type="button" class="ff-btn ff-btn--primary" class:ff-btn--disabled={!changed} disabled={!changed} onclick={save}>Save</button>
  {/snippet}
</Popover>

<style>
  .ff-bpmpop__row {
    display: flex;
    align-items: center;
    gap: var(--ff-space-4);
  }
  .ff-bpmpop__input {
    flex: 1;
    min-width: 0;
    height: 28px;
    padding: 0 var(--ff-space-4);
    box-sizing: border-box;
    border: 1px solid var(--ff-accent);
    border-radius: var(--ff-radius-md);
    background: var(--ff-bg);
    caret-color: var(--ff-accent);
    color: var(--ff-text);
    font: var(--ff-type-body) var(--ff-font-mono);
    font-variant-numeric: tabular-nums;
    outline: none;
  }
  .ff-bpmpop__input--invalid {
    border-color: var(--ff-danger);
  }
  .ff-bpmpop__input::selection {
    background: color-mix(in srgb, var(--ff-accent) 22%, transparent);
    color: inherit;
  }
  .ff-bpmpop__scale {
    display: inline-flex;
    height: 28px;
    box-sizing: border-box;
    border: 1px solid var(--ff-border-hi);
    border-radius: var(--ff-radius-md);
    overflow: hidden;
  }
  .ff-bpmpop__scale button {
    width: 30px;
    padding: 0;
    border: none;
    background: transparent;
    color: var(--ff-text-mid);
    font: 600 var(--ff-type-num) var(--ff-font-mono);
    cursor: pointer;
  }
  .ff-bpmpop__scale button + button {
    border-left: 1px solid var(--ff-border);
  }
  .ff-bpmpop__scale button:hover {
    background: var(--ff-hover);
    color: var(--ff-text);
  }
</style>
