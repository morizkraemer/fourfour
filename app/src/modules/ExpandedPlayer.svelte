<!--
  ExpandedPlayer.svelte — docked ~1/3-height player panel.

  Hosts the dual-view WaveformView (overview preview + Rekordbox playhead-locked
  zoom view). Carries the same transport as the compact bar (CUE, play, BPM, key,
  times) plus zoom controls for the waveform. Seeking routes through the same
  `seekToProgress` path as the compact strip, so playback/scrub behaviour matches.
-->
<script>
  import { WaveformView, Button, Nudge } from '$ds';
  import {
    player,
    currentTime,
    currentTimeMs,
    totalTime,
    togglePlay,
    seekToProgress,
    cuePreviewStart,
    cuePreviewStop,
    toggleExpanded,
    beatGridNudgeLeft,
    beatGridNudgeRight,
    beatGridSetCurrent,
  } from '../stores/player.svelte.ts';

  let wave = $state(null);

  const stop = (fn) => (e) => {
    e.stopPropagation();
    fn?.(e);
  };
</script>

<svelte:window
  on:keydown={(e) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      toggleExpanded();
    }
  }} />

<div class="ff-xp" role="group" aria-label="Expanded player">
  <header class="ff-xp__head">
    <div class="ff-xp__meta">
      {#if player.track?.cover}
        <img class="ff-xp__cover" src={player.track.cover} alt="" />
      {:else}
        <div class="ff-xp__cover ff-xp__cover--empty"></div>
      {/if}
      <div class="ff-xp__titles">
        <span class="ff-xp__title">{player.track?.title || 'No Track Loaded'}</span>
        <span class="ff-xp__artist">{player.track?.artist || '—'}</span>
      </div>
    </div>

    <div class="ff-xp__head-right">
      <Button label={player.track?.key || '—'} variant="default" size="compact" />
      <Nudge value={player.track?.bpm || '—'} label="BPM" variant="compact" />
      <div class="ff-xp__times">
        <span class="ff-xp__time-cur">{currentTime()}<span class="ff-xp__time-ms">{currentTimeMs()}</span></span>
        <span class="ff-xp__time-tot">{totalTime()}</span>
      </div>
      <button
        class="ff-xp__collapse"
        title="Collapse player (Esc)"
        aria-label="Collapse player"
        onclick={toggleExpanded}>
        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor"
          stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="6 9 12 15 18 9" />
        </svg>
      </button>
    </div>
  </header>

  <div class="ff-xp__wave">
    <WaveformView
      bind:this={wave}
      mode="expanded"
      colorData={player.track?.colorData ?? null}
      colorDetail={player.track?.colorDetail ?? null}
      colorOverview={player.track?.colorOverview ?? null}
      previewBytes={player.track?.previewBytes ?? null}
      cues={player.track?.cues ?? []}
      beats={player.track?.beats ?? []}
      bpm={player.track?.bpm ?? 0}
      durationMs={player.track?.durationMs ?? 0}
      trackKey={player.track?.id ?? null}
      progress={player.progress}
      playing={player.playing}
      onSeek={player.track ? seekToProgress : undefined} />
  </div>

  <footer class="ff-xp__transport">
    <div class="ff-xp__transport-left">
      <button
        class="ff-xp__cue-btn"
        title="Cue (hold to preview)"
        aria-label="Cue"
        onpointerdown={stop(cuePreviewStart)}
        onpointerup={stop(cuePreviewStop)}
        onpointercancel={stop(cuePreviewStop)}
        onpointerleave={stop(cuePreviewStop)}>
        <span class="ff-xp__cue-txt">CUE</span>
      </button>
      <button
        class="ff-xp__play"
        title={player.playing ? 'Pause' : 'Play'}
        aria-label={player.playing ? 'Pause' : 'Play'}
        disabled={!player.track}
        onclick={() => togglePlay()}>
        {#if player.playing}
          <svg viewBox="0 0 24 24" width="13" height="13" fill="currentColor"><rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/></svg>
        {:else}
          <svg viewBox="0 0 24 24" width="13" height="13" fill="currentColor"><polygon points="6,4 6,20 18,12"/></svg>
        {/if}
      </button>
    </div>

    <div class="ff-xp__grid-edit" role="group" aria-label="Beat grid">
      <span class="ff-xp__grid-label">GRID</span>
      <button class="ff-xp__grid-btn" title="Nudge grid left" aria-label="Nudge grid left"
        disabled={!player.track} onclick={() => beatGridNudgeLeft()}>◀</button>
      <button class="ff-xp__grid-btn" title="Set downbeat to playhead" aria-label="Set downbeat to playhead"
        disabled={!player.track} onclick={() => beatGridSetCurrent()}>Set</button>
      <button class="ff-xp__grid-btn" title="Nudge grid right" aria-label="Nudge grid right"
        disabled={!player.track} onclick={() => beatGridNudgeRight()}>▶</button>
    </div>

    <div class="ff-xp__zoom">
      <button class="ff-xp__zoom-btn" title="Zoom out" aria-label="Zoom out" onclick={() => wave?.zoomOut()}>−</button>
      <button class="ff-xp__zoom-btn" title="Fit" aria-label="Reset zoom" onclick={() => wave?.resetZoom()}>Fit</button>
      <button class="ff-xp__zoom-btn" title="Zoom in" aria-label="Zoom in" onclick={() => wave?.zoomIn()}>+</button>
    </div>
  </footer>
</div>

<style>
  .ff-xp {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    min-height: 0;
    background: var(--ff-bg, #0d0d0d);
  }

  .ff-xp__head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--ff-space-4, 16px);
    padding: 10px 14px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    flex-shrink: 0;
  }

  .ff-xp__meta {
    display: flex;
    align-items: center;
    gap: var(--ff-space-3, 12px);
    min-width: 0;
  }
  .ff-xp__cover {
    width: 40px;
    height: 40px;
    border-radius: var(--ff-radius-sm, 4px);
    object-fit: cover;
    flex-shrink: 0;
  }
  .ff-xp__cover--empty { background: var(--ff-elev, rgba(255, 255, 255, 0.06)); }
  .ff-xp__titles { display: flex; flex-direction: column; min-width: 0; gap: 1px; }
  .ff-xp__title {
    font-weight: 500;
    font-size: 13px;
    color: var(--ff-text, #fff);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .ff-xp__artist {
    font-size: 11.5px;
    color: var(--ff-muted, rgba(255, 255, 255, 0.6));
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }

  .ff-xp__head-right { display: flex; align-items: center; gap: 10px; }
  .ff-xp__times { display: flex; flex-direction: column; align-items: flex-end; line-height: 1.15; }
  .ff-xp__time-cur { font-variant-numeric: tabular-nums; color: var(--ff-text, #fff); font-size: 13px; }
  .ff-xp__time-ms { color: var(--ff-muted, rgba(255, 255, 255, 0.5)); }
  .ff-xp__time-tot { font-variant-numeric: tabular-nums; color: var(--ff-muted, rgba(255, 255, 255, 0.5)); font-size: 11px; }

  .ff-xp__collapse {
    display: inline-flex; align-items: center; justify-content: center;
    width: 30px; height: 30px;
    border: none; border-radius: var(--ff-radius-sm, 4px);
    background: transparent; color: var(--ff-muted, rgba(255, 255, 255, 0.6));
    cursor: pointer;
  }
  .ff-xp__collapse:hover { background: var(--ff-elev, rgba(255, 255, 255, 0.06)); color: var(--ff-text, #fff); }

  .ff-xp__wave { flex: 1; min-height: 0; position: relative; }

  .ff-xp__transport {
    display: flex; align-items: center; justify-content: space-between;
    gap: var(--ff-space-4, 16px);
    padding: 10px 14px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    flex-shrink: 0;
  }
  .ff-xp__transport-left { display: flex; align-items: center; gap: 10px; }

  /* CUE / play match the compact player: circular, orange/green strokes. */
  .ff-xp__cue-btn {
    display: inline-flex; align-items: center; justify-content: center;
    width: 30px; height: 30px; padding: 0;
    border: 1px solid #f59e0b; border-radius: 50%;
    background: transparent; cursor: pointer;
    transition: background-color var(--ff-motion-fast, 120ms) ease;
  }
  .ff-xp__cue-btn:hover { background-color: rgba(245, 158, 11, 0.08); }
  .ff-xp__cue-txt {
    font-family: var(--ff-font-mono, monospace);
    font-size: 8px; font-weight: 500; color: #d4d4d4; line-height: 1;
  }

  .ff-xp__play {
    display: inline-flex; align-items: center; justify-content: center;
    width: 30px; height: 30px; padding: 0;
    border: 1px solid #22c55e; border-radius: 50%;
    background: transparent; color: #d4d4d4; cursor: pointer;
    transition: background-color var(--ff-motion-fast, 120ms) ease;
  }
  .ff-xp__play:hover { background-color: rgba(34, 197, 94, 0.08); }
  .ff-xp__play:disabled { opacity: 0.4; cursor: default; }

  /* Beat-grid edit cluster */
  .ff-xp__grid-edit { display: flex; align-items: center; gap: 4px; }
  .ff-xp__grid-label {
    font-family: var(--ff-font-mono, monospace); font-size: 9px;
    letter-spacing: 0.08em; color: var(--ff-muted, rgba(255, 255, 255, 0.5));
    margin-right: 2px;
  }
  .ff-xp__grid-btn {
    height: 26px; min-width: 30px; padding: 0 8px;
    border: 1px solid var(--ff-border, rgba(255, 255, 255, 0.12));
    border-radius: var(--ff-radius-sm, 4px);
    background: transparent; color: var(--ff-text, #fff);
    font-size: 11px; line-height: 1; cursor: pointer;
  }
  .ff-xp__grid-btn:hover { background: var(--ff-elev, rgba(255, 255, 255, 0.08)); }
  .ff-xp__grid-btn:disabled { opacity: 0.4; cursor: default; }

  .ff-xp__zoom { display: flex; align-items: center; gap: 4px; }
  .ff-xp__zoom-btn {
    height: 26px; min-width: 30px; padding: 0 8px;
    border: 1px solid var(--ff-border, rgba(255, 255, 255, 0.12));
    border-radius: var(--ff-radius-sm, 4px);
    background: transparent; color: var(--ff-text, #fff);
    font-size: 13px; line-height: 1; cursor: pointer;
  }
  .ff-xp__zoom-btn:hover { background: var(--ff-elev, rgba(255, 255, 255, 0.08)); }
</style>
