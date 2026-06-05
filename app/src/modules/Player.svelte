<!--
  Player.svelte — Compact player bar module.
  Wraps the $ds PlayerCompact primitive with player store state.
-->
<script>
  import { PlayerCompact } from '$ds';
  import ExpandedPlayer from './ExpandedPlayer.svelte';
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
  } from '../stores/player.svelte.ts';

  let playerRoot;
</script>

<div class="ff-player-module" class:ff-player-module--expanded={player.expanded && player.track} bind:this={playerRoot}>
  {#if player.expanded && player.track}
    <ExpandedPlayer />
  {:else}
  <PlayerCompact
    cover={player.track?.cover ?? ''}
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
    onScrub={player.track ? seekToProgress : undefined}
    onPlayToggle={player.track ? togglePlay : undefined}
    onCueDown={player.track ? cuePreviewStart : undefined}
    onCueUp={player.track ? cuePreviewStop : undefined}
    onExpand={player.track ? toggleExpanded : undefined}
  />
  {/if}
</div>

<style>
  .ff-player-module {
    width: 100%;
    background: var(--ff-bg);
    border-top: 1px solid var(--ff-border);
  }
  /* Expanded: a docked panel ~1/3 of the viewport height. */
  .ff-player-module--expanded {
    height: 33vh;
    min-height: 280px;
    display: flex;
  }
</style>
