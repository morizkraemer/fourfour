<script>
  import { DetailPane, ContextMenu } from '$ds';
  import {
    selectionCount,
    selectedTrack,
  } from '../stores/selection.svelte.ts';
  import { ensureTrackPeaksLoaded, isTrackAnalyzing, revealFile } from '../stores/library.svelte.ts';
  import { player } from '../stores/player.svelte.ts';
  import { getAnalysisData } from '../services/tauri.svelte.ts';
  import { stripBandsFromAnalysis } from '../services/waveform-bands.ts';
  import { cuesOf } from '../stores/cues.svelte.ts';
  import {
    addHotCueAtPlayhead,
    addMemoryCueAtPlayhead,
    convertCue,
    deleteCue,
    recolorCue,
    renameCue,
  } from '../stores/cue-actions.ts';
  import { CUE_PALETTE, cueChip, cueMarkers } from '../lib/cues.ts';
  import { buildRevealItem } from '../menus/context-menus.ts';
  import { seekTrackToMs, seekTrackToProgress } from '../stores/inspector-seek.ts';
  import { commitTrackBpm, commitTrackFields, commitTrackKey, fitGridToBpm, releaseTrackEdits } from '../stores/track-edits.svelte.ts';

  let mode = $derived(
    selectionCount() === 0 ? 'empty' :
    selectionCount() === 1 ? 'single' : 'multi'
  );

  let track = $derived(selectedTrack());
  let trackIsLoaded = $derived(!!track && player.track?.id === track.id);

  let cueChips = $derived(cuesOf(track?.id).map(cueChip));

  function addCue(kind) {
    if (kind === 'memory') addMemoryCueAtPlayhead();
    else addHotCueAtPlayhead();
  }

  let pathMenu = $state({ open: false, x: 0, y: 0 });

  function openPathMenu(e) {
    pathMenu = { open: true, x: e.clientX, y: e.clientY };
  }

  $effect(() => {
    const current = track;
    if (!current) return;
    void ensureTrackPeaksLoaded(current);
  });

  let stripBands = $state(null);

  $effect(() => {
    const current = track;
    void current?.bandsPreview;
    void current?.analysisPending;
    stripBands = null;
    if (!current || !current.analyzed) return;
    const id = current.id;
    let stale = false;
    getAnalysisData(id)
      .then((analysis) => {
        if (!stale) stripBands = stripBandsFromAnalysis(analysis);
      })
      .catch(() => {});
    return () => {
      stale = true;
    };
  });

  let tempoEditable = $derived(!!track && track.analyzed === true && track.analysisPending !== true && !!track.bpmExact);
  let hasEdits = $derived(!!track && (track.bpmLocked || track.keyLocked));

  let bpmMenuItems = $derived(track ? [
    {
      label: `Adjust beat grid to ${Number(track.bpmExact).toFixed(2)} BPM`,
      onClick: () => fitGridToBpm(track.id, track.bpmExact),
    },
    ...(hasEdits ? [{ label: 'Let re-analysis replace BPM and key', onClick: () => releaseTrackEdits(track.id) }] : []),
  ] : []);
  let keyMenuItems = $derived(hasEdits && track ? [
    { label: 'Let re-analysis replace BPM and key', onClick: () => releaseTrackEdits(track.id) },
  ] : []);

  let fields = $derived(track ? [
    { key: 'album', label: 'Album', value: track.album === '—' ? '' : track.album, placeholder: 'None' },
    { key: 'genre', label: 'Genre', value: track.genre === '—' ? '' : track.genre, placeholder: 'None' },
  ] : []);

  let durationMs = $derived((track?.raw?.duration_secs ?? 0) * 1000);
  let markers = $derived(cueMarkers(cuesOf(track?.id), durationMs));

  let multiMetaObj = $derived({
    'TOTAL SIZE': `${selectionCount()} files`,
    'TOTAL DURATION': '—',
    'BPM RANGE': '—',
    'KEYS': '—',
  });
</script>

<div class="ff-detail-module">
  {#if mode === 'single' && track}
    {#key track.id}
    <DetailPane
      {mode}
      title={track.title}
      artist={track.artist}
      {fields}
      onFieldChange={(key, value) => commitTrackFields(track.id, { [key]: value })}
      duration={track.time ?? ''}
      bpmLocked={track.bpmLocked}
      keyLocked={track.keyLocked}
      onReleaseLocks={() => releaseTrackEdits(track.id)}
      {markers}
      onMarkerClick={(marker) => seekTrackToMs(track, marker.timeMs)}
      cues={cueChips}
      cuePalette={CUE_PALETTE}
      showCues={track.analyzed}
      canSetCues={trackIsLoaded}
      canJumpToCues
      onCueAdd={addCue}
      onCueJump={(chip) => seekTrackToMs(track, chip.timeMs)}
      onCueRename={(chip, name) => renameCue(track.id, chip.key, name)}
      onCueRecolor={(chip, color) => recolorCue(track.id, chip.key, color)}
      onCueDelete={(chip) => deleteCue(track.id, chip.key)}
      onCueConvert={(chip) => convertCue(track.id, chip.key)}
      filePath={track.filePath ?? ''}
      cover={track.cover ?? ''}
      bands={stripBands}
      analyzing={isTrackAnalyzing(track.id)}
      onWaveformSeek={(progress) => seekTrackToProgress(track, progress)}
      onRevealPath={() => track.filePath && revealFile(track.filePath)}
      onPathContextMenu={openPathMenu}
      bpm={track.bpmExact ?? 0}
      musicalKey={track.key ?? ''}
      onBpmChange={tempoEditable ? (next) => commitTrackBpm(track.id, next) : undefined}
      onKeyChange={tempoEditable ? (next) => commitTrackKey(track.id, next) : undefined}
      {bpmMenuItems}
      {keyMenuItems}
    />
    {/key}
  {:else if mode === 'multi'}
    <DetailPane
      {mode}
      multiCount={selectionCount()}
      multiMeta={multiMetaObj}
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
