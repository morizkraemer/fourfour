<!--
  Detail.svelte — Right detail pane module.
  Reads selection store and forwards to the $ds DetailPane primitive.
  Supports empty / single / multi modes.
-->
<script>
  import { DetailPane } from '$ds';
  import {
    selectionCount,
    selectedTrack,
    selectedTracks,
  } from '../stores/selection.svelte.ts';

  // Derive the detail pane mode from selection state
  let mode = $derived(
    selectionCount() === 0 ? 'empty' :
    selectionCount() === 1 ? 'single' : 'multi'
  );

  // Single-track metadata for the detail pane
  let track = $derived(selectedTrack());

  let singleMeta = $derived(track ? {
    ALBUM: track.label ?? '—',
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

<style>
  .ff-detail-module {
    width: 278px;
    height: 100%;
    background: var(--ff-bg);
    border-left: 1px solid var(--ff-border);
    overflow-y: auto;
    overflow-x: hidden;
  }
  .ff-detail-module > :global(.ff-detail-pane) {
    width: 100%;
    height: 100%;
    border-left: none; /* module owns the divider */
  }
</style>
