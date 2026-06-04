<script>
  // Compact player (Svelte port of player-compact.js). The full-width resting
  // player: cue + play, track meta, BPM nudge + key toggle, times, expand, and a
  // CSS-bar waveform with played overlay / playhead / hot-cue markers.
  import './player-compact.css';
  import Button from './Button.svelte';
  import Nudge from './Nudge.svelte';

  let {
    title,
    artist,
    currentTime,
    currentTimeMs = '.2',
    totalTime,
    progress = 0,
    cues = [],
    playing = false,
    bpm = '124.00',
    key = '8A',
    onPlayToggle,
    onCue,
    onBpmChange,
    onKeyToggle,
    onExpand,
  } = $props();

  const stop = (fn) => (e) => {
    e.stopPropagation();
    fn?.(e);
  };

  let pct = $derived(`${progress * 100}%`);
</script>

<div class="ff-player-compact">
  <div class="ff-player-compact__left-controls">
    <button class="ff-player-compact__cue-btn" onclick={stop(onCue)}>
      <span class="ff-player-compact__cue-txt">CUE</span>
    </button>
    <button class="ff-player-compact__play-btn" onclick={stop(onPlayToggle)}>
      {#if playing}
        <svg class="ff-player-compact__play-icon" viewBox="0 0 24 24" width="11" height="11" fill="currentColor">
          <rect x="6" y="4" width="4" height="16" /><rect x="14" y="4" width="4" height="16" />
        </svg>
      {:else}
        <svg class="ff-player-compact__play-icon" viewBox="0 0 24 24" width="11" height="11" fill="currentColor">
          <polygon points="6,4 6,20 18,12" />
        </svg>
      {/if}
    </button>
  </div>

  <div class="ff-player-compact__content">
    <div class="ff-player-compact__meta-row">
      <div class="ff-player-compact__title-group">
        <span class="ff-player-compact__meta-title">{title}</span>
        <span class="ff-player-compact__meta-artist">{artist}</span>
      </div>
      <div class="ff-player-compact__right-toggles">
        <div class="ff-player-compact__bpm-key-group">
          <Button
            label={key}
            variant="default"
            size="compact"
            class="ff-player-compact__key-btn"
            onclick={stop(onKeyToggle)} />
          <Nudge value={bpm} label="BPM" onChange={onBpmChange} variant="compact" />
        </div>
        <div class="ff-player-compact__times-group">
          <span class="ff-player-compact__time-cur">
            {currentTime}<span class="ff-player-compact__time-cur-ms">{currentTimeMs}</span>
          </span>
          <span class="ff-player-compact__time-tot">{totalTime}</span>
        </div>
        <button class="ff-player-compact__expand-btn" onclick={stop(onExpand)}>
          <svg
            class="ff-player-compact__expand-chevron"
            viewBox="0 0 24 24"
            width="12"
            height="12"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round">
            <polyline points="18 15 12 9 6 15" />
          </svg>
        </button>
      </div>
    </div>

    <div class="ff-player-compact__waveform">
      <div class="ff-player-compact__waveform-played" style="width:{pct}"></div>
      {#each cues as cue}
        {@const left = `${Math.max(0, Math.min(1, cue.position ?? 0)) * 100}%`}
        {@const color = cue.color || '#4ade80'}
        <div class="ff-player-compact__waveform-cue" style="left:{left};background-color:{color}">
          <span class="ff-player-compact__waveform-cue-dot" style="background-color:{color}"></span>
        </div>
      {/each}
      <div class="ff-player-compact__waveform-playhead" style="left:{pct}"></div>
    </div>
  </div>
</div>
