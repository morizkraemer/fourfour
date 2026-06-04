<!--
  TrackTable.svelte — Library track table module.
  Column header + scrollable track rows with click-to-select.
  Matches module-table.artboard.js structure.
-->
<script>
  import { ColumnHeader, TrackRow } from '$ds';
  import { tracks, TABLE_COLUMNS } from '../stores/library.svelte.ts';
  import { selection, select, toggle } from '../stores/selection.svelte.ts';

  function handleRowClick(index, event) {
    if (event.metaKey || event.ctrlKey) {
      toggle(index);
    } else {
      select(index);
    }
  }

  function rowState(index) {
    const track = tracks[index];
    if (track?.bpm === '—') return 'analyzing';
    if (selection.indices.has(index)) return 'selected';
    return 'rest';
  }
</script>

<div class="ff-table">
  <ColumnHeader columns={TABLE_COLUMNS} />
  <div class="ff-table__body">
    {#each tracks as track, i}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div onclick={(e) => handleRowClick(i, e)}>
        <TrackRow
          {track}
          state={rowState(i)}
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
