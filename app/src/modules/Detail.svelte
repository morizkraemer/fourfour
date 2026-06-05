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
  } from '../stores/selection.svelte.ts';
  import { ensureTrackPeaksLoaded } from '../stores/library.svelte.ts';
  import { getTrackArtwork } from '../services/tauri.svelte.ts';

  let mode = $derived(
    selectionCount() === 0 ? 'empty' :
    selectionCount() === 1 ? 'single' : 'multi'
  );

  let track = $derived(selectedTrack());

  $effect(() => {
    const current = track;
    if (!current) return;
    void ensureTrackPeaksLoaded(current);

    if (current.raw?.has_artwork && !current.cover && !current.loadingArtwork) {
      current.loadingArtwork = true;
      getTrackArtwork(current.id)
        .then((bytes) => {
          if (bytes && bytes.length > 0) {
            const blob = new Blob([new Uint8Array(bytes)], { type: 'image/jpeg' });
            current.cover = URL.createObjectURL(blob);
          }
        })
        .catch((err) => {
          console.warn('Failed to fetch track artwork:', err);
        })
        .finally(() => {
          current.loadingArtwork = false;
        });
    }
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
      peaks={track.peaks}
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
