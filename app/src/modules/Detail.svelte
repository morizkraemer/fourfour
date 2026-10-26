<!--
  Detail.svelte — Right detail pane module.
  Reads selection store and forwards to the $ds DetailPane primitive.
  Supports empty / single / multi modes.
-->
<script>
  import { DetailPane, ContextMenu } from '$ds';
  import {
    selectionCount,
    selectedTrack,
  } from '../stores/selection.svelte.ts';
  import { ensureTrackPeaksLoaded, revealFile } from '../stores/library.svelte.ts';
  import { player, seekToProgress } from '../stores/player.svelte.ts';
  import { buildRevealItem } from '../menus/context-menus.ts';

  let mode = $derived(
    selectionCount() === 0 ? 'empty' :
    selectionCount() === 1 ? 'single' : 'multi'
  );

  let track = $derived(selectedTrack());
  let trackIsLoaded = $derived(!!track && player.track?.id === track.id);

  let pathMenu = $state({ open: false, x: 0, y: 0 });

  function openPathMenu(e) {
    pathMenu = { open: true, x: e.clientX, y: e.clientY };
  }

  $effect(() => {
    const current = track;
    if (!current) return;
    void ensureTrackPeaksLoaded(current);
  });

  let singleMeta = $derived(track ? {
    ALBUM: track.album ?? '—',
    LABEL: track.label ?? '—',
    BPM: track.bpm ?? '—',
    KEY: track.key ?? '—',
    DURATION: track.time ?? '—',
    ADDED: '2 days ago',
  } : {});

  let multiMetaObj = $derived({
    'TOTAL SIZE': `${selectionCount()} files`,
    'TOTAL DURATION': '—',
    'BPM RANGE': '—',
    'KEYS': '—',
  });
</script>

<div class="ff-detail-module">
  {#if mode === 'single' && track}
    <DetailPane
      {mode}
      title={track.title}
      artist={track.artist}
      meta={singleMeta}
      cues={track.cues ?? []}
      filePath={track.filePath ?? ''}
      cover={track.cover ?? ''}
      bands={track.bands}
      onWaveformSeek={trackIsLoaded ? seekToProgress : undefined}
      onRevealPath={() => track.filePath && revealFile(track.filePath)}
      onPathContextMenu={openPathMenu}
    />
  {:else if mode === 'multi'}
    <DetailPane
      {mode}
      multiCount={selectionCount()}
      meta={multiMetaObj}
    />
  {:else}
    <DetailPane mode="empty" />
  {/if}
</div>

<ContextMenu
  open={pathMenu.open}
  x={pathMenu.x}
  y={pathMenu.y}
  items={[buildRevealItem(track?.filePath)]}
  onClose={() => (pathMenu = { ...pathMenu, open: false })}
/>

<style>
  .ff-detail-module {
    width: 100%;
    height: 100%;
    /* Inspector is the grey mid-layer between the black browse window and the drawer. */
    background: var(--ff-surface);
    border-left: 1px solid var(--ff-border);
    overflow-y: auto;
    overflow-x: hidden;
  }
  .ff-detail-module > :global(.ff-detail-pane) {
    width: 100%;
    height: 100%;
    border-left: none;
  }
</style>
