<!--
  PlayerTempo.svelte — the player's tempo and key controls.

  These change what is played, not what is stored: the BPM field shows the BPM being played
  (analysed BPM × tempo), the KEY field shows the key being played (analysed key shifted by
  semitones). The analysed values stay in the inspector.
-->
<script>
  import { Nudge, ScrubValue, SegmentedControl, keyColor } from '$ds';
  import { player } from '../stores/player.svelte.ts';
  import {
    tempo,
    tempoActive,
    resetTempo,
    setKeyLock,
    setKeyShift,
    setTempoPercent,
    setTempoRange,
  } from '../stores/tempo.svelte.ts';
  import {
    TEMPO_RANGES,
    formatShift,
    percentForBpm,
    playingBpm,
    playingBpmScrub,
    keyShiftScrub,
    shiftedKey,
    tempoPercentScrub,
  } from '../lib/tempo.ts';

  const MARKED = 'var(--ff-status-warn)';

  let analysedBpm = $derived(Number(player.track?.bpm));
  let analysedKey = $derived(player.track?.key ?? '');
  let hasBpm = $derived(Number.isFinite(analysedBpm) && analysedBpm > 0);
  let bpmNow = $derived(hasBpm ? playingBpm(analysedBpm, tempo.percent) : NaN);
  let bpmModel = $derived(playingBpmScrub(hasBpm ? analysedBpm : 1, tempo.range));
  let percentModel = $derived(tempoPercentScrub(tempo.range));
  let keyModel = $derived(keyShiftScrub(analysedKey, tempo.keyShift));
  let keyNow = $derived(shiftedKey(analysedKey, tempo.keyShift) ?? '');
  let off = $derived(tempoActive());
  let rangeLabels = TEMPO_RANGES.map((range) => `±${range}`);
</script>

<div class="ff-ptempo" class:ff-ptempo--idle={!player.track} role="group" aria-label="Tempo and key">
  <SegmentedControl
    items={rangeLabels}
    activeIndex={TEMPO_RANGES.indexOf(tempo.range)}
    onChange={(index) => setTempoRange(TEMPO_RANGES[index])} />
  <button
    type="button"
    class="ff-ptempo__mt"
    class:ff-ptempo__mt--on={tempo.keyLock}
    aria-pressed={tempo.keyLock}
    title="Master Tempo: keep the key when the tempo changes"
    onclick={() => setKeyLock(!tempo.keyLock)}>MT</button>
  <span class="ff-ptempo__percent" title="Tempo offset">
    <ScrubValue
      model={percentModel}
      value={tempo.percent}
      label="Tempo"
      color={tempo.percent !== 0 ? MARKED : undefined}
      onCommit={setTempoPercent} />
  </span>
  {#if off}
    <button
      type="button"
      class="ff-ptempo__reset"
      title="Back to the track's own tempo and key"
      aria-label="Reset tempo and key"
      onclick={resetTempo}>
      <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M3 12a9 9 0 1 0 3-6.7" /><polyline points="3 4 3 10 9 10" />
      </svg>
    </button>
  {/if}
  <div class="ff-ptempo__field">
    <Nudge
      label="KEY"
      variant="compact"
      model={keyModel}
      value={tempo.keyShift}
      color={tempo.keyShift !== 0 ? MARKED : keyColor(keyNow)}
      onCommit={setKeyShift} />
    {#if tempo.keyShift !== 0}
      <span class="ff-ptempo__chip" title="Key shift">{formatShift(tempo.keyShift)}</span>
    {/if}
  </div>
  <div class="ff-ptempo__field">
    <Nudge
      label="BPM"
      variant="compact"
      model={bpmModel}
      value={bpmNow}
      color={tempo.percent !== 0 ? MARKED : undefined}
      onCommit={(bpm) => setTempoPercent(percentForBpm(bpm, analysedBpm))} />
    {#if hasBpm && tempo.percent !== 0}
      <span class="ff-ptempo__chip" title="Analysed BPM">orig {analysedBpm.toFixed(2)}</span>
    {/if}
  </div>
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
  .ff-ptempo :global(.ff-segmented) {
    padding: 1px;
  }
  .ff-ptempo :global(.ff-segmented__item) {
    padding: 3px 6px;
    font-size: 10.5px;
    font-family: var(--ff-font-mono);
  }
  .ff-ptempo__mt {
    height: 22px;
    padding: 0 7px;
    border: 1px solid var(--ff-border-hi);
    border-radius: var(--ff-radius-sm);
    background: var(--ff-bg);
    color: var(--ff-muted);
    font-family: var(--ff-font-mono);
    font-size: 10.5px;
    font-weight: 600;
    cursor: pointer;
  }
  .ff-ptempo__mt:hover {
    color: var(--ff-text);
  }
  .ff-ptempo__mt--on {
    background: var(--ff-elev-hi);
    color: var(--ff-text);
  }
  .ff-ptempo__percent {
    display: inline-flex;
    min-width: 54px;
    justify-content: center;
    font-family: var(--ff-font-mono);
    font-size: 11px;
  }
  .ff-ptempo__reset {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border: 1px solid var(--ff-border-hi);
    border-radius: var(--ff-radius-sm);
    background: var(--ff-bg);
    color: var(--ff-status-warn);
    cursor: pointer;
  }
  .ff-ptempo__reset:hover {
    background: var(--ff-hover);
  }
  .ff-ptempo__field {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .ff-ptempo__chip {
    font-family: var(--ff-font-mono);
    font-size: 10px;
    color: var(--ff-muted);
    white-space: nowrap;
  }
</style>
