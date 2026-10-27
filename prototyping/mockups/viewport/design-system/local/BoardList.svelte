<script>
  import { ColumnHeader, TrackRow } from '../../../../viewport/design-system/svelte.js';
  import { PLAYLIST_ROWS } from './data.js';

  let { tracks = PLAYLIST_ROWS, dropAfter = -1, columns = null, selectedId = -1, header = true, placeholderRow = false } = $props();
  const DEFAULT = [
    { key: 'index', label: '#', width: 24 },
    { key: 'cover', label: '', width: 22 },
    { key: 'title', label: 'TITLE', flex: true },
    { key: 'bpm', label: 'BPM', width: 48, align: 'right' },
    { key: 'key', label: 'KEY', width: 32, align: 'right' },
  ];
  let cols = $derived(columns ?? DEFAULT);
  let list = $derived(tracks.map((t, i) => ({ ...t, index: i + 1 })));
</script>

<div class="bl">
  {#if header}<ColumnHeader columns={cols} />{/if}
  {#each list as track, i (track.id)}
    <TrackRow {track} columns={cols} state={track.id === selectedId ? 'selected' : 'rest'} />
    {#if dropAfter === i}
      <div class="bl__drop"><span></span></div>
    {/if}
  {/each}
  {#if placeholderRow}
    <div class="bl__ph">Drop here to add</div>
  {/if}
</div>

<style>
  .bl { position: relative; }
  .bl__drop { position: relative; height: 0; z-index: 2; }
  .bl__drop span { position: absolute; left: 10px; right: 10px; top: -1px; height: 2px; background: var(--ff-accent); border-radius: 1px; box-shadow: 0 0 0 3px rgba(255, 255, 255, 0.08); }
  .bl__ph { margin: 8px 12px; height: 32px; border: 1px dashed var(--ff-border-hi); border-radius: var(--ff-radius-md); display: flex; align-items: center; justify-content: center; font-size: 12px; color: var(--ff-muted); }
</style>
