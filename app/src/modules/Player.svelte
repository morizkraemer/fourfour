<!--
  Player.svelte — Compact player bar module.
  Wraps the $ds PlayerCompact primitive with player store state.
-->
<script>
  import { PlayerCompact, ResizeHandle } from '$ds';
  import ExpandedPlayer from './ExpandedPlayer.svelte';
  import { cuesOf } from '../stores/cues.svelte.ts';
  import { cueMarkers } from '../lib/cues.ts';
  import { ui, setPlayerHeight, commitPlayerHeight, resetPlayerHeight } from '../stores/ui.svelte.ts';
  import { PLAYER_HEIGHT, clampPlayerHeight, defaultPlayerHeight, maxPlayerHeight } from '../lib/playerHeight.ts';
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

  let cueMarkerList = $derived(cueMarkers(cuesOf(player.track?.id), player.track?.durationMs ?? 0));

  let playerRoot;
  let viewportHeight = $state(window.innerHeight);
  let expandedHeight = $derived(
    ui.playerHeight == null ? defaultPlayerHeight(viewportHeight) : clampPlayerHeight(ui.playerHeight, viewportHeight),
  );
</script>

<svelte:window bind:innerHeight={viewportHeight} />

<div
  class="ff-player-module"
  class:ff-player-module--expanded={player.expanded && player.track}
  style:height={player.expanded && player.track ? `${expandedHeight}px` : undefined}
  bind:this={playerRoot}>
  {#if player.expanded && player.track}
    <ResizeHandle
      edge="top"
      label="Resize player"
      value={expandedHeight}
      min={PLAYER_HEIGHT.min}
      max={maxPlayerHeight(viewportHeight)}
      onResize={(px) => setPlayerHeight(px, viewportHeight)}
      onCommit={commitPlayerHeight}
      onReset={resetPlayerHeight}
    />
    <ExpandedPlayer />
  {:else}
  <PlayerCompact
    cover={player.cover}
    title={player.track?.title || 'No Track Loaded'}
    artist={player.track?.artist || '—'}
    playing={player.playing}
    progress={player.progress}
    currentTime={currentTime()}
    currentTimeMs={currentTimeMs()}
    totalTime={totalTime()}
    bpm={player.track?.bpm || '—'}
    key={player.track?.key || '—'}
    cues={cueMarkerList}
    bands={player.track?.bands ?? null}
    bandsOverview={player.track?.bandsOverview ?? null}
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
    position: relative;
    width: 100%;
    background: var(--ff-bg);
    border-top: 1px solid var(--ff-border);
  }
  .ff-player-module--expanded {
    display: flex;
  }
</style>
