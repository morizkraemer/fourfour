<!--
  Player.svelte — Compact player bar module.
  Wraps the $ds PlayerCompact primitive with player store state.
-->
<script>
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

  $effect(() => {
    const track = selectedTrack();
    if (!track) {
      clearPlayerTrack();
      return;
    }
    void loadPlayerTrack(track);
  });
</script>

<div class="ff-player-module">
  <PlayerCompact
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
