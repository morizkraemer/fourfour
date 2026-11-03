<!--
  Player.svelte — Compact player bar module.
  Wraps the $ds PlayerCompact primitive with player store state.
-->
<script>
  import { PlayerCompact, ResizeHandle } from '$ds';
  import { isTrackAnalyzing } from '../stores/library.svelte.ts';
  import { waveSliderProps } from '../stores/wave-seek.ts';
  import ExpandedPlayer from './ExpandedPlayer.svelte';
  import PlayerTempo from './PlayerTempo.svelte';
  import { cuesOf } from '../stores/cues.svelte.ts';
  import { cueMarkers } from '../lib/cues.ts';
  import { dnd } from '../stores/dnd.svelte.ts';
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
  class:ff-player-module--drop={dnd.kind === 'tracks' && dnd.dropPlayer}
  data-drop-player
  class:ff-player-module--expanded={player.expanded}
  style:height={player.expanded ? `${expandedHeight}px` : undefined}
  bind:this={playerRoot}>
  {#if player.expanded}
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
    cues={cueMarkerList}
    bands={player.track?.bands ?? null}
    bandsOverview={player.track?.bandsOverview ?? null}
    beats={player.track?.beats ?? []}
    durationMs={player.track?.durationMs ?? 0}
    trackKey={player.track?.id ?? null}
    analyzing={player.track ? isTrackAnalyzing(player.track.id) : false}
    disabled={!player.track}
    onScrub={player.track ? seekToProgress : undefined}
    waveformProps={waveSliderProps()}
    onPlayToggle={player.track ? togglePlay : undefined}
    onCueDown={player.track ? cuePreviewStart : undefined}
    onCueUp={player.track ? cuePreviewStop : undefined}
    onExpand={toggleExpanded}>
    {#snippet controls()}<PlayerTempo />{/snippet}
  </PlayerCompact>
  {/if}
</div>

<style>
  .ff-player-module {
    position: relative;
    width: 100%;
    background: var(--ff-bg);
    border-top: 1px solid var(--ff-border);
  }
  .ff-player-module--drop {
    box-shadow: inset 0 2px 0 var(--ff-accent);
  }
  .ff-player-module--expanded {
    display: flex;
  }
</style>
