<!--
  Player.svelte — Compact player bar module.
  Wraps the $ds PlayerCompact primitive with player store state.
-->
<script>
  import { onMount } from 'svelte';
  import { PlayerCompact } from '$ds';
  import { selectedTrack } from '../stores/selection.svelte.ts';
  import {
    player,
    currentTime,
    currentTimeMs,
    totalTime,
    togglePlay,
    seekToProgress,
    loadPlayerTrack,
    clearPlayerTrack,
  } from '../stores/player.svelte.ts';

  let playerRoot;
  let waveformStrip = $state(null);

  $effect(() => {
    const track = selectedTrack();
    if (!track) {
      clearPlayerTrack();
      return;
    }
    void loadPlayerTrack(track);
  });

  function isEditableTarget(target) {
    if (!(target instanceof Element)) return false;
    if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) return true;
    if (target.closest('[contenteditable="true"]')) return true;
    return !!target.closest('dialog[open]');
  }

  function onPlayerKeyDown(e) {
    if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.altKey) return;
    if (isEditableTarget(e.target)) return;
    if (!player.track) return;

    if (e.key === ' ' || e.code === 'Space') {
      e.preventDefault();
      void togglePlay();
      return;
    }

    const renderer = waveformStrip?.getRenderer?.();
    if (renderer?.handleKey(e)) {
      e.preventDefault();
    }
  }

  onMount(() => {
    window.addEventListener('keydown', onPlayerKeyDown);
    return () => window.removeEventListener('keydown', onPlayerKeyDown);
  });
</script>

<div class="ff-player-module" bind:this={playerRoot}>
  <PlayerCompact
    bind:waveformStrip
    cover={selectedTrack()?.cover ?? ''}
    title={player.track?.title || 'No Track Loaded'}
    artist={player.track?.artist || '—'}
    playing={player.playing}
    progress={player.progress}
    currentTime={currentTime()}
    currentTimeMs={currentTimeMs()}
    totalTime={totalTime()}
    bpm={player.track?.bpm || '—'}
    key={player.track?.key || '—'}
    cues={player.track?.cues || []}
    colorData={player.track?.colorData ?? null}
    previewBytes={player.track?.previewBytes ?? null}
    beats={player.track?.beats ?? []}
    durationMs={player.track?.durationMs ?? 0}
    trackKey={player.track?.id ?? null}
    followPlayhead={player.playing}
    onScrub={player.track ? seekToProgress : undefined}
    onPlayToggle={player.track ? togglePlay : undefined}
  />
</div>

<style>
  .ff-player-module {
    width: 100%;
    background: var(--ff-bg);
    border-top: 1px solid var(--ff-border);
  }
</style>
