<!--
  ExpandedPlayer.svelte — full-screen maximized player.

  Hosts the colleague's dual-view WaveformDisplay (overview bar + scrollable zoom
  view, Rekordbox-calibrated 3-band color, beat grid, click-seek). The compact
  strip (Player.svelte) stays as-is; this is the opt-in expanded surface.

  Data flows from the player store: `colorDetail` feeds the zoom view, `colorOverview`
  the overview bar. The store keeps rgb in 0–1; WaveformDisplay expects 0–255, so we
  scale here. Seeking routes through the same `seekToProgress` path as the compact
  strip, so playback/scrub behaviour is identical.
-->
<script>
  import { onMount, onDestroy } from 'svelte';
  import WaveformDisplay from '../lib/WaveformDisplay.js';
  import {
    player,
    currentTime,
    currentTimeMs,
    totalTime,
    togglePlay,
    seekToProgress,
    toggleExpanded,
  } from '../stores/player.svelte.ts';

  let container = $state(null);
  let display = $state(null);
  let resizeObs = null;
  let lastDataKey = null;

  // WaveformDisplay's bandAmps divides weights by 255; the store keeps rgb in 0–1.
  function toDisplay(samples) {
    if (!samples) return [];
    return samples.map((s) => ({ amp: s.amp, r: s.r * 255, g: s.g * 255, b: s.b * 255 }));
  }

  function buildData(track) {
    if (!track) return null;
    const detail = track.colorDetail ?? track.colorData;
    const overview = track.colorOverview ?? track.colorData;
    return {
      waveform_color: toDisplay(detail),
      waveform_overview: overview ? toDisplay(overview) : undefined,
      waveform_preview: track.previewBytes ?? undefined,
      beats: track.beats ?? [],
      duration_ms: track.durationMs ?? 0,
      bpm: Number(track.bpm) || 0,
    };
  }

  onMount(() => {
    display = new WaveformDisplay(container);
    display.onSeek = (frac) => seekToProgress(frac);
    resizeObs = new ResizeObserver(() => display?.redraw());
    resizeObs.observe(container);
  });

  onDestroy(() => {
    resizeObs?.disconnect();
    display?.destroy();
    display = null;
  });

  // Push data only when the loaded track or its waveform changes — NOT on every
  // playback tick (setData resets zoom/scroll). `display` is reactive so this
  // also runs once onMount assigns it.
  $effect(() => {
    const t = player.track;
    const key = t
      ? `${t.id}:${t.colorDetail?.length ?? 0}:${t.colorOverview?.length ?? 0}:${t.beats?.length ?? 0}:${t.durationMs}`
      : null;
    if (!display) return;
    if (key === lastDataKey) return;
    lastDataKey = key;
    if (t) display.setData(buildData(t));
    else display.clear();
  });

  // Playhead follows playback progress (cheap overlay update, no re-render).
  $effect(() => {
    const frac = player.track ? player.progress : -1;
    display?.setPlayhead(frac);
  });

  function onKeydown(e) {
    if (e.key === 'Escape') {
      e.preventDefault();
      toggleExpanded();
    }
  }
</script>

<svelte:window on:keydown={onKeydown} />

<div class="ff-expanded-player" role="dialog" aria-label="Expanded player" aria-modal="true">
  <header class="ff-expanded-player__bar">
    <div class="ff-expanded-player__meta">
      {#if player.track?.cover}
        <img class="ff-expanded-player__cover" src={player.track.cover} alt="" />
      {:else}
        <div class="ff-expanded-player__cover ff-expanded-player__cover--empty"></div>
      {/if}
      <div class="ff-expanded-player__titles">
        <span class="ff-expanded-player__title">{player.track?.title || 'No Track Loaded'}</span>
        <span class="ff-expanded-player__artist">{player.track?.artist || '—'}</span>
      </div>
    </div>
    <div class="ff-expanded-player__stats">
      <span class="ff-expanded-player__stat"><b>{player.track?.bpm || '—'}</b> BPM</span>
      <span class="ff-expanded-player__stat"><b>{player.track?.key || '—'}</b> KEY</span>
    </div>
    <button
      class="ff-expanded-player__close"
      title="Collapse player (Esc)"
      aria-label="Collapse player"
      onclick={toggleExpanded}>
      <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor"
        stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="6 9 12 15 18 9" />
      </svg>
    </button>
  </header>

  <div class="ff-expanded-player__wave" bind:this={container}></div>

  <footer class="ff-expanded-player__transport">
    <button
      class="ff-expanded-player__play"
      aria-label={player.playing ? 'Pause' : 'Play'}
      disabled={!player.track}
      onclick={() => togglePlay()}>
      {#if player.playing}
        <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor"><rect x="6" y="5" width="4" height="14" rx="1"/><rect x="14" y="5" width="4" height="14" rx="1"/></svg>
      {:else}
        <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor"><path d="M8 5v14l11-7z"/></svg>
      {/if}
    </button>
    <span class="ff-expanded-player__time">
      {currentTime()}<span class="ff-expanded-player__time-ms">{currentTimeMs()}</span>
      <span class="ff-expanded-player__time-sep">/</span>
      {totalTime()}
    </span>
  </footer>
</div>

<style>
  .ff-expanded-player {
    position: fixed;
    inset: 0;
    z-index: 1000;
    display: flex;
    flex-direction: column;
    background: var(--ff-bg, #0d0d0d);
    padding-top: env(titlebar-area-height, 0);
  }

  .ff-expanded-player__bar {
    display: flex;
    align-items: center;
    gap: var(--ff-space-4, 16px);
    padding: var(--ff-space-3, 12px) var(--ff-space-4, 16px);
    border-bottom: 1px solid var(--ff-border, rgba(255, 255, 255, 0.08));
  }

  .ff-expanded-player__meta {
    display: flex;
    align-items: center;
    gap: var(--ff-space-3, 12px);
    min-width: 0;
    flex: 1;
  }

  .ff-expanded-player__cover {
    width: 44px;
    height: 44px;
    border-radius: var(--ff-radius-sm, 4px);
    object-fit: cover;
    flex-shrink: 0;
  }
  .ff-expanded-player__cover--empty {
    background: var(--ff-surface, rgba(255, 255, 255, 0.06));
  }

  .ff-expanded-player__titles {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .ff-expanded-player__title {
    font-weight: 600;
    color: var(--ff-text, #fff);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ff-expanded-player__artist {
    font-size: 0.85em;
    color: var(--ff-text-dim, rgba(255, 255, 255, 0.6));
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ff-expanded-player__stats {
    display: flex;
    gap: var(--ff-space-4, 16px);
    color: var(--ff-text-dim, rgba(255, 255, 255, 0.6));
    font-size: 0.8em;
  }
  .ff-expanded-player__stat b {
    color: var(--ff-text, #fff);
    font-size: 1.15em;
    margin-right: 3px;
  }

  .ff-expanded-player__close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border: none;
    border-radius: var(--ff-radius-sm, 4px);
    background: transparent;
    color: var(--ff-text-dim, rgba(255, 255, 255, 0.6));
    cursor: pointer;
  }
  .ff-expanded-player__close:hover {
    background: var(--ff-surface, rgba(255, 255, 255, 0.06));
    color: var(--ff-text, #fff);
  }

  .ff-expanded-player__wave {
    flex: 1;
    min-height: 0;
    position: relative;
  }

  .ff-expanded-player__transport {
    display: flex;
    align-items: center;
    gap: var(--ff-space-4, 16px);
    padding: var(--ff-space-3, 12px) var(--ff-space-4, 16px);
    border-top: 1px solid var(--ff-border, rgba(255, 255, 255, 0.08));
  }
  .ff-expanded-player__play {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 40px;
    border: none;
    border-radius: 50%;
    background: var(--ff-accent, #4ade80);
    color: #000;
    cursor: pointer;
  }
  .ff-expanded-player__play:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .ff-expanded-player__time {
    font-variant-numeric: tabular-nums;
    color: var(--ff-text, #fff);
  }
  .ff-expanded-player__time-ms {
    color: var(--ff-text-dim, rgba(255, 255, 255, 0.5));
  }
  .ff-expanded-player__time-sep {
    margin: 0 6px;
    color: var(--ff-text-dim, rgba(255, 255, 255, 0.4));
  }

  /* WaveformDisplay builds its own DOM (unscoped) — style globally. */
  :global(.ff-expanded-player__wave .waveform-display) {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    background: #0d0d0d;
    overflow: hidden;
  }
  :global(.ff-expanded-player__wave .waveform-display__overview) {
    display: block;
    width: 100%;
    height: 48px;
    cursor: pointer;
    flex-shrink: 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }
  :global(.ff-expanded-player__wave .waveform-display__zoom) {
    display: block;
    width: 100%;
    height: 100%;
    cursor: default;
  }
</style>
