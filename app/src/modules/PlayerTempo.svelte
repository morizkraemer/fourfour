<!--
  PlayerTempo.svelte — the player's tempo and key controls.

  These change what is played, not what is stored: the BPM field shows the BPM being played
  (analysed BPM × tempo), the KEY field shows the key being played (analysed key shifted by
  semitones). The analysed values stay in the inspector and the expanded player.
-->
<script lang="ts">
  import { keyColor } from '$ds';
  import TempoField from './TempoField.svelte';
  import { player } from '../stores/player.svelte.ts';
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
    parseKeyEntry,
    percentAfterBpmStep,
    percentForBpm,
    playingBpm,
    shiftedKey,
    type Tier,
  } from '../lib/tempo.ts';

  const MARKED = 'var(--ff-status-warn)';

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
</script>

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
</style>
