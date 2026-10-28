<!--
  PlayerTempo.svelte — the player's tempo and key controls.

  These change what is played, not what is stored: the BPM field shows the BPM being played
  (analysed BPM × tempo), the KEY field shows the key being played (analysed key shifted by
  semitones). The analysed values stay in the inspector and the expanded player.
-->
<script lang="ts">
  import { Icon, keyColor } from '$ds';
  import TempoField from './TempoField.svelte';
  import { player, setTrackMode } from '../stores/player.svelte.ts';
  import { bpmEntry, commitTrackBpm, commitTrackKey, doubleTrackBpm, halveTrackBpm, stepTrackBpm, stepTrackKey } from '../stores/track-edits.svelte.ts';
  import { tempo, setKeyLock, setKeyShift, setTempoPercent } from '../stores/tempo.svelte.ts';
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
    camelotStep,
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

  let trackBpm = $derived(analysedBpm);
  let bpmTrackMode = $derived(expanded && player.trackBpmMode);
  let keyTrackMode = $derived(expanded && player.trackKeyMode);

  function trackKeyEntry(typed: string): boolean {
    const key = camelotStep(typed.trim().toUpperCase(), 0);
    if (key === null || !player.track) return false;
    void commitTrackKey(player.track.id, key);
    keyDone();
    return true;
  }

  function trackBpmEntry(typed: string): boolean {
    const bpm = parseBpmEntry(typed);
    if (bpm === null || !player.track || !commitTrackBpm(player.track.id, bpm)) return false;
    setTrackMode('bpm', false);
    return true;
  }

  function keyDone() {
    setTrackMode('key', false);
  }
</script>

{#if expanded}
  <div class="ff-ptempo ff-ptempo--xp" class:ff-ptempo--idle={!player.track} role="group" aria-label="Tempo and key">
    <div class="ff-ptempo__group">
      <TempoField
        tall
        label={keyTrackMode ? 'Track key' : 'Key'}
        text={hasKey ? (keyTrackMode ? analysedKey : keyNow) : '—'}
        width="5ch"
        pxPerUnit={DRAG_PX_PER_UNIT.key}
        color={keyColor(keyTrackMode ? analysedKey : keyNow)}
        disabled={!hasKey}
        editText={hasKey ? (keyTrackMode ? analysedKey : keyNow) : ''}
        onStep={(units) => (keyTrackMode ? void stepTrackKey(player.track!.id, units) : stepKey(units))}
        onSet={keyTrackMode ? trackKeyEntry : setKey}
        onReset={() => !keyTrackMode && setKeyShift(0)} />
      <button
        type="button"
        class="ff-ptempo__pen"
        class:ff-ptempo__pen--on={keyTrackMode}
        aria-pressed={keyTrackMode}
        aria-label={keyTrackMode ? 'Finish editing track key' : 'Edit track key'}
        title={keyTrackMode ? 'Back to pitch (Esc)' : 'Edit the track key'}
        disabled={!hasKey}
        onclick={() => setTrackMode('key', !keyTrackMode)}>
        {#if keyTrackMode}<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><polyline points="4 12 10 18 20 6" /></svg>{:else}<Icon name="pencil" size={13} />{/if}
      </button>
    </div>
    <div class="ff-ptempo__group">
      <span class="ff-ptempo__slot">
        {#if bpmTrackMode}
          <button type="button" class="ff-ptempo__slot-btn" aria-label="Halve track BPM" title="Halve the track BPM"
            disabled={!hasBpm} onclick={() => halveTrackBpm(player.track!.id)}>÷2</button>
          <button type="button" class="ff-ptempo__slot-btn" aria-label="Double track BPM" title="Double the track BPM"
            disabled={!hasBpm} onclick={() => doubleTrackBpm(player.track!.id)}>×2</button>
        {:else}
          <button
            type="button"
            class="ff-ptempo__slot-btn ff-ptempo__mt"
            class:ff-ptempo__mt--on={tempo.keyLock}
            aria-pressed={tempo.keyLock}
            title="Master Tempo: keep the key when the tempo changes"
            onclick={() => setKeyLock(!tempo.keyLock)}>MT</button>
        {/if}
      </span>
      <TempoField
        tall
        label={bpmTrackMode ? 'Track BPM' : 'BPM'}
        text={bpmTrackMode ? (hasBpm ? trackBpm.toFixed(2) : '—') : formatBpm(bpmNow)}
        width="8ch"
        pxPerUnit={DRAG_PX_PER_UNIT.bpm}
        color={!bpmTrackMode && bpmDiffers(analysedBpm, bpmNow) ? MARKED : undefined}
        disabled={!hasBpm}
        editText={hasBpm ? (bpmTrackMode ? trackBpm : bpmNow).toFixed(2) : ''}
        editSignal={bpmEntry.request}
        onStep={(units, tier, fromButton) => (bpmTrackMode ? void stepTrackBpm(player.track!.id, units, tier) : stepBpm(units, tier, fromButton))}
        onSet={bpmTrackMode ? trackBpmEntry : setBpm}
        onReset={() => !bpmTrackMode && setTempoPercent(0)} />
      <button
        type="button"
        class="ff-ptempo__pen"
        class:ff-ptempo__pen--on={bpmTrackMode}
        aria-pressed={bpmTrackMode}
        aria-label={bpmTrackMode ? 'Finish editing track BPM' : 'Edit track BPM'}
        title={bpmTrackMode ? 'Back to pitch (Esc)' : 'Edit the track BPM (moves the grid, no pitch)'}
        disabled={!hasBpm}
        onclick={() => setTrackMode('bpm', !bpmTrackMode)}>
        {#if bpmTrackMode}<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><polyline points="4 12 10 18 20 6" /></svg>{:else}<Icon name="pencil" size={13} />{/if}
      </button>
    </div>
  </div>
{:else}
<div class="ff-ptempo" class:ff-ptempo--idle={!player.track} role="group" aria-label="Tempo and key">
    <button
      type="button"
      class="ff-ptempo__mt"
      class:ff-ptempo__mt--on={tempo.keyLock}
      aria-pressed={tempo.keyLock}
      title="Master Tempo: keep the key when the tempo changes"
      onclick={() => setKeyLock(!tempo.keyLock)}>MT</button>
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
  .ff-ptempo__mt {
    flex: none;
    padding: 3px 4px;
    border: none;
    border-radius: var(--ff-radius-xs);
    background: none;
    color: var(--ff-text);
    font-family: var(--ff-font-mono);
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.04em;
    cursor: pointer;
    transition: color var(--ff-motion-fast) var(--ff-easing), background-color var(--ff-motion-fast) var(--ff-easing);
  }
  .ff-ptempo__mt:hover {
    background: var(--ff-hover);
  }
  .ff-ptempo__mt:focus-visible {
    outline: 1px solid var(--ff-border-hi);
  }
  .ff-ptempo__mt--on {
    color: var(--ff-danger);
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
  .ff-ptempo__slot-btn {
    flex: 1;
    min-width: 0;
    padding: 0;
    border: none;
    border-radius: var(--ff-radius-md);
    background: none;
    color: var(--ff-text-mid);
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-num);
    font-weight: 600;
    cursor: pointer;
  }
  .ff-ptempo__slot-btn:hover:not(:disabled) {
    background: var(--ff-hover);
    color: var(--ff-text);
  }
  .ff-ptempo__slot-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .ff-ptempo__slot .ff-ptempo__mt--on {
    color: var(--ff-danger);
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
