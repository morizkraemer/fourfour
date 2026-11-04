<!--
  PlayerTempo.svelte — the player's tempo and key controls.

  These change what is played, not what is stored: the BPM field shows the BPM being played
  (analysed BPM × tempo), the KEY field shows the key being played (analysed key shifted by
  semitones). The analysed values stay in the inspector and the expanded player.
-->
<script lang="ts">
  import { Icon, keyColor } from '$ds';
  import TempoField from './TempoField.svelte';
  import LazyMount from '../lib/LazyMount.svelte';
  import { trackBpmPopup, trackKeyPopup } from '../lib/lazy-views.ts';
  import { endTrackModes, player, setTrackMode } from '../stores/player.svelte.ts';
    import { tempo, setKeyLock, setKeyShift, setTempoPercent } from '../stores/tempo.svelte.ts';
  import { quantize, setQuantize } from '../stores/quantize.svelte.ts';
  import {
    BPM_CLICK_STEP,
    BPM_CLICK_STEP_FINE,
    DRAG_PX_PER_UNIT,
    bpmDiffers,
    bpmDragStep,
    clampShift,
    effectiveShift,
    formatBpm,
    parseBpmEntry,
    parseKeyEntry,
    percentAfterBpmStep,
    percentForBpm,
    playingBpm,
    shiftedKey,
    type Tier,
  } from '../lib/tempo.ts';

  const MARKED = 'var(--ff-status-warn)';

  let { expanded = false }: { expanded?: boolean } = $props();

  let analysedBpm = $derived(Number(player.track?.bpm));
  let analysedKey = $derived(player.track?.key ?? '');
  let hasBpm = $derived(Number.isFinite(analysedBpm) && analysedBpm > 0);
  let hasKey = $derived(shiftedKey(analysedKey, 0) !== null);
  let bpmNow = $derived(hasBpm ? playingBpm(analysedBpm, tempo.percent) : NaN);
  let pitchShift = $derived(effectiveShift(tempo.keyShift, tempo.percent, tempo.keyLock));
  let keyNow = $derived(shiftedKey(analysedKey, pitchShift) ?? '');

  function stepBpm(units: number, tier: Tier, fromButton: boolean) {
    if (!hasBpm) return;
    const bpmStep = fromButton ? (tier === 'fine' ? BPM_CLICK_STEP_FINE : BPM_CLICK_STEP) : bpmDragStep(tier);
    setTempoPercent(percentAfterBpmStep(analysedBpm, tempo.percent, units, bpmStep));
  }

  function setBpm(typed: string): boolean {
    const bpm = parseBpmEntry(typed);
    if (bpm === null || !hasBpm) return false;
    setTempoPercent(percentForBpm(bpm, analysedBpm));
    return true;
  }

  function stepKey(units: number) {
    if (hasKey) setKeyShift(clampShift(tempo.keyShift + units));
  }

  function setKey(typed: string): boolean {
    const shift = parseKeyEntry(typed, analysedKey, pitchShift);
    if (shift === null) return false;
    setKeyShift(shift - (pitchShift - tempo.keyShift));
    return true;
  }

  let bpmTrackMode = $derived(expanded && player.trackBpmMode);
  let keyTrackMode = $derived(expanded && player.trackKeyMode);

  let keyPen: HTMLElement | undefined = $state();
  let bpmPen: HTMLElement | undefined = $state();

  function openTrackEdit(field: 'bpm' | 'key') {
    const open = field === 'bpm' ? bpmTrackMode : keyTrackMode;
    endTrackModes();
    setTrackMode(field, !open);
  }
</script>

{#if expanded}
  <div class="ff-ptempo ff-ptempo--xp" class:ff-ptempo--idle={!player.track} role="group" aria-label="Tempo and key">
    <div class="ff-ptempo__group">
      <TempoField
        tall
        label="Key"
        text={hasKey ? keyNow : '—'}
        width="5ch"
        pxPerUnit={DRAG_PX_PER_UNIT.key}
        color={keyColor(keyNow)}
        disabled={!hasKey}
        editText={hasKey ? keyNow : ''}
        onStep={stepKey}
        onSet={setKey}
        onReset={() => setKeyShift(0)} />
      <button
        type="button"
        class="ff-ptempo__pen"
        class:ff-ptempo__pen--on={keyTrackMode}
        bind:this={keyPen}
        aria-pressed={keyTrackMode}
        aria-haspopup="dialog"
        aria-label="Edit track key"
        title="Edit the track key"
        disabled={!hasKey}
        onclick={() => openTrackEdit('key')}>
        <Icon name="pencil" size={13} />
      </button>
    </div>
    <div class="ff-ptempo__group">
      <span class="ff-ptempo__slot">
        <button
          type="button"
          class="ff-ptempo__toggle ff-ptempo__toggle--fill"
          aria-pressed={tempo.keyLock}
          disabled={!player.track}
          title="Master Tempo: keep the key when the tempo changes"
          onclick={() => setKeyLock(!tempo.keyLock)}>MT</button>
        <button
          type="button"
          class="ff-ptempo__toggle ff-ptempo__toggle--fill"
          aria-pressed={quantize.on}
          disabled={!player.track}
          title="Quantize: hot cues and CUE snap to the beat grid (hold Alt while dragging a cue to place it freely)"
          onclick={() => setQuantize(!quantize.on)}>Q</button>
      </span>
      <TempoField
        tall
        label="BPM"
        text={formatBpm(bpmNow)}
        width="8ch"
        pxPerUnit={DRAG_PX_PER_UNIT.bpm}
        color={bpmDiffers(analysedBpm, bpmNow) ? MARKED : undefined}
        disabled={!hasBpm}
        editText={hasBpm ? bpmNow.toFixed(2) : ''}
        onStep={stepBpm}
        onSet={setBpm}
        onReset={() => setTempoPercent(0)} />
      <button
        type="button"
        class="ff-ptempo__pen"
        class:ff-ptempo__pen--on={bpmTrackMode}
        bind:this={bpmPen}
        aria-pressed={bpmTrackMode}
        aria-haspopup="dialog"
        aria-label="Edit track BPM"
        title="Edit the track BPM (moves the grid, no pitch)"
        disabled={!hasBpm}
        onclick={() => openTrackEdit('bpm')}>
        <Icon name="pencil" size={13} />
      </button>
    </div>
  </div>
  {#if keyTrackMode}<LazyMount view={trackKeyPopup} props={{ anchor: keyPen }} />{/if}
  {#if bpmTrackMode}<LazyMount view={trackBpmPopup} props={{ anchor: bpmPen }} />{/if}
{:else}
<div class="ff-ptempo" class:ff-ptempo--idle={!player.track} role="group" aria-label="Tempo and key">
    <button
      type="button"
      class="ff-ptempo__toggle"
      aria-pressed={tempo.keyLock}
      disabled={!player.track}
      title="Master Tempo: keep the key when the tempo changes"
      onclick={() => setKeyLock(!tempo.keyLock)}>MT</button>
    <button
      type="button"
      class="ff-ptempo__toggle"
      aria-pressed={quantize.on}
      disabled={!player.track}
      title="Quantize: hot cues and CUE snap to the beat grid"
      onclick={() => setQuantize(!quantize.on)}>Q</button>
    <TempoField
      label="Key"
      text={hasKey ? keyNow : '—'}
      width="5ch"
      pxPerUnit={DRAG_PX_PER_UNIT.key}
      color={keyColor(keyNow)}
      disabled={!hasKey}
      editText={hasKey ? keyNow : ''}
      onStep={stepKey}
      onSet={setKey}
      onReset={() => setKeyShift(0)} />
    <TempoField
      label="BPM"
      text={formatBpm(bpmNow)}
      width="8ch"
      pxPerUnit={DRAG_PX_PER_UNIT.bpm}
      color={bpmDiffers(analysedBpm, bpmNow) ? MARKED : undefined}
      disabled={!hasBpm}
      editText={hasBpm ? bpmNow.toFixed(2) : ''}
      onStep={stepBpm}
      onSet={setBpm}
      onReset={() => setTempoPercent(0)} />
  </div>
{/if}

<style>
  .ff-ptempo {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .ff-ptempo--idle {
    opacity: 0.45;
    pointer-events: none;
  }
  .ff-ptempo__toggle {
    flex: none;
    padding: 3px 4px;
    border: none;
    border-radius: var(--ff-radius-xs);
    background: none;
    color: var(--ff-text-mid);
    font-family: var(--ff-font-mono);
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.04em;
    cursor: pointer;
    transition: color var(--ff-motion-fast) var(--ff-easing), background-color var(--ff-motion-fast) var(--ff-easing);
  }
  .ff-ptempo__toggle:hover:not(:disabled) {
    background: var(--ff-hover);
  }
  .ff-ptempo__toggle:active:not(:disabled) {
    background: var(--ff-select);
  }
  .ff-ptempo__toggle:focus-visible {
    outline: 1px solid var(--ff-border-hi);
  }
  .ff-ptempo__toggle[aria-pressed='true'] {
    color: var(--ff-toggle-on, #3b82f6);
  }
  .ff-ptempo__toggle:disabled {
    cursor: default;
  }
  .ff-ptempo__toggle--fill {
    flex: 1;
    min-width: 0;
    padding: 0;
    border-radius: var(--ff-radius-md);
    font-size: var(--ff-type-num);
  }
  .ff-ptempo--xp {
    gap: var(--ff-space-8);
  }
  .ff-ptempo__group {
    display: inline-flex;
    align-items: center;
    gap: var(--ff-space-4);
  }
  .ff-ptempo__slot {
    display: inline-flex;
    width: 58px;
    height: 28px;
    flex: none;
  }
  .ff-ptempo__pen {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: 28px;
    height: 28px;
    padding: 0;
    border: none;
    border-radius: var(--ff-radius-md);
    background: none;
    color: var(--ff-muted);
    cursor: pointer;
  }
  .ff-ptempo__pen:hover:not(:disabled) {
    background: var(--ff-hover);
    color: var(--ff-text);
  }
  .ff-ptempo__pen--on {
    background: var(--ff-select);
    color: var(--ff-text);
  }
  .ff-ptempo__pen:disabled {
    opacity: 0.4;
    cursor: default;
  }
</style>
