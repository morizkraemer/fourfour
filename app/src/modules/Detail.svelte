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
  import { getAnalysisData, getTrackArtwork } from '../services/tauri.svelte.ts';

  // Derive the detail pane mode from selection state
  let mode = $derived(
    selectionCount() === 0 ? 'empty' :
    selectionCount() === 1 ? 'single' : 'multi'
  );

  // Single-track metadata for the detail pane
  let track = $derived(selectedTrack());

  // Fetch analysis data and artwork dynamically when selected track changes
  $effect(() => {
    const current = track;
    if (current && !current.peaksLoaded && !current.loadingAnalysis) {
      current.loadingAnalysis = true;
      
      // Load Waveform & Cues
      getAnalysisData(current.id).then(res => {
        if (res.waveform_preview) {
          current.peaks = Array.from(res.waveform_preview).map(byte => (byte & 0x1F) / 31.0);
        }
        if (res.cue_points) {
          current.cues = res.cue_points.map((cue, idx) => {
            const mins = Math.floor(cue.time_ms / 60000);
            const secs = Math.floor((cue.time_ms % 60000) / 1000);
            const posStr = `${mins}:${String(secs).padStart(2, '0')}`;
            return {
              name: cue.hot_cue_number === 0 ? 'Memory Cue' : `Hot Cue ${String.fromCharCode(64 + cue.hot_cue_number)}`,
              position: posStr,
              color: cue.color
            };
          });
        }
        current.peaksLoaded = true;
      }).catch(err => {
        console.warn('Failed to fetch analysis details:', err);
      }).finally(() => {
        current.loadingAnalysis = false;
      });

      // Load Artwork Image
      getTrackArtwork(current.id).then(bytes => {
        if (bytes && bytes.length > 0) {
          const blob = new Blob([new Uint8Array(bytes)], { type: 'image/jpeg' });
          current.cover = URL.createObjectURL(blob);
        }
      }).catch(err => {
        console.warn('Failed to fetch track artwork:', err);
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
