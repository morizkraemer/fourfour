<script>
  // Column header row (Svelte port of column-header.js). Driven by the same
  // `columns` array as TrackRow so widths stay aligned.
  //   columns: [{ label, width?, flex?, align?, sort?:'asc'|'desc' }]
  import './column-header.css';
  import Icon from './Icon.svelte';

  let { columns = [] } = $props();

  function cellStyle(col) {
    const base = col.flex ? 'flex:1 1 0' : `width:${col.width}px;flex:none`;
    return col.align === 'right' ? `${base};justify-content:flex-end` : base;
  }
</script>

<div class="ff-colh">
  {#each columns as col}
    <div class="ff-colh__cell{col.sort ? ' ff-colh__cell--active' : ''}" style={cellStyle(col)}>
      <span class="ff-colh__label">{col.label ?? ''}</span>
      {#if col.sort}
        <Icon
          name="chevron-down"
          size={10}
          strokeWidth={2.25}
          class={'ff-colh__arrow' + (col.sort === 'asc' ? ' ff-colh__arrow--asc' : '')} />
      {/if}
    </div>
  {/each}
</div>
