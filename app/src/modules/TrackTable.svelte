<!--
  TrackTable.svelte — Library track table module.
  Column header + scrollable track rows with click-to-select.
  Matches module-table.artboard.js structure.
-->
<script>
  import { ColumnHeader, TrackRow } from '$ds';
  import { library, TABLE_COLUMNS } from '../stores/library.svelte.ts';
  import { selection, select, toggle, selectRange } from '../stores/selection.svelte.ts';
  import { ui } from '../stores/ui.svelte.ts';

  let lastSelectedId = null;

  // Compute what tracks to show based on sidebar selection
  let currentTracks = $derived.by(() => {
    const active = ui.activeSidebarRow;
    if (!active || active === 'All Tracks' || active === 'all-tracks') {
      return library.tracks;
    }
    if (active === 'Recently Added') {
      return library.tracks.filter(t => t.raw?.tempo > 0);
    }
    if (active === 'Unfiled') {
      return library.tracks.filter(t => t.raw?.tempo === 0);
    }
    // Check if it matches a USB path
    if (library.volumes.includes(active)) {
      return library.usbTracks;
    }
    // Check if it's a playlist name
    const playlist = library.playlists.find(p => p.name === active);
    if (playlist) {
      return library.tracks.filter(t => playlist.track_ids.includes(t.id));
    }
    // Check if it's a favorite category
    if (active === 'Peak Time' || active === 'Warmup') {
      const pl = library.playlists.find(p => p.name === active);
      if (pl) {
        return library.tracks.filter(t => pl.track_ids.includes(t.id));
      }
    }
    return library.tracks;
  });

  function handleRowClick(track, event) {
    if (event.shiftKey && lastSelectedId !== null) {
      selectRange(lastSelectedId, track.id, currentTracks);
    } else if (event.metaKey || event.ctrlKey) {
      toggle(track.id);
      lastSelectedId = track.id;
    } else {
      select(track.id);
      lastSelectedId = track.id;
    }
  }

  function rowState(track) {
    if (track?.bpm === '—') return 'analyzing';
    if (selection.ids.has(track.id)) return 'selected';
    return 'rest';
  }
</script>

<div class="ff-table">
  <ColumnHeader columns={TABLE_COLUMNS} />
  <div class="ff-table__body">
    {#each currentTracks as track, i}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div onclick={(e) => handleRowClick(track, e)}>
        <TrackRow
          {track}
          state={rowState(track)}
          columns={TABLE_COLUMNS}
        />
      </div>
    {/each}
  </div>
</div>

<style>
  .ff-table {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    background: var(--ff-bg);
  }
  .ff-table__body {
    flex: 1 1 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }
  .ff-table__body > :global(div) {
    cursor: default;
  }
</style>
