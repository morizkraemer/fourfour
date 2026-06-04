<!--
  Player.svelte — Compact player bar module.
  Wraps the $ds PlayerCompact primitive with player store state.
-->
<script>
  import { PlayerCompact } from '$ds';
  import {
    player,
    currentTime,
    currentTimeMs,
    totalTime,
    togglePlay,
  } from '../stores/player.svelte.ts';
</script>

<div class="ff-player-module">
  <PlayerCompact
    title={player.track.title}
    artist={player.track.artist}
    playing={player.playing}
    progress={player.progress}
    currentTime={currentTime()}
    currentTimeMs={currentTimeMs()}
    totalTime={totalTime()}
    bpm={player.track.bpm}
    key={player.track.key}
    cues={player.track.cues}
    onPlayToggle={togglePlay}
  />
</div>

<style>
  .ff-player-module {
    width: 100%;
    background: var(--ff-bg);
    border-top: 1px solid var(--ff-border);
  }
</style>
