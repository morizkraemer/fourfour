<script>
  // Column header row (Svelte port of column-header.js). Driven by the same
  // `columns` array as TrackRow so widths stay aligned.
  //   columns: [{ label, width?, flex?, align?, sort?:'asc'|'desc', key? }]
  import './column-header.css';
  import Icon from './Icon.svelte';

  let {
    columns = [],
    columnDragKey = null,
    columnDropKey = null,
    onColumnContextMenu,
    onColumnClick,
    onColumnReorder,
    onColumnDragChange,
  } = $props();

  function cellStyle(col) {
    const base = col.flex ? 'flex:1 1 0' : `width:${col.width}px;flex:none`;
    return col.align === 'right' ? `${base};justify-content:flex-end` : base;
  }

  let activeCol = null;
  let startX = 0;
  let startWidth = 0;

  function startResize(e, col) {
    e.preventDefault();
    e.stopPropagation();
    activeCol = col;
    startX = e.clientX;
    startWidth = col.width || 0;

    window.addEventListener('mousemove', handleResize);
    window.addEventListener('mouseup', stopResize);
  }

  function handleResize(e) {
    if (!activeCol) return;
    const deltaX = e.clientX - startX;
    activeCol.width = Math.max(15, startWidth + deltaX);
  }

  function stopResize() {
    activeCol = null;
    window.removeEventListener('mousemove', handleResize);
    window.removeEventListener('mouseup', stopResize);
  }

  function onLabelPointerDown(e, col) {
    if (!col.key || e.button !== 0) return;
    e.preventDefault();
    const startKey = col.key;
    let dragging = false;

    function onMove(ev) {
      if (!dragging && (Math.abs(ev.clientX - e.clientX) > 4 || Math.abs(ev.clientY - e.clientY) > 4)) {
        dragging = true;
        onColumnDragChange?.(startKey, null);
      }
      if (!dragging) return;
      const el = document.elementFromPoint(ev.clientX, ev.clientY);
      const cell = el?.closest?.('[data-col-key]');
      const hoverKey = cell?.getAttribute?.('data-col-key');
      if (hoverKey && hoverKey !== startKey) {
        onColumnDragChange?.(startKey, hoverKey);
      }
    }

    function onUp(ev) {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      if (dragging && columnDropKey && columnDropKey !== startKey) {
        onColumnReorder?.(startKey, columnDropKey);
      }
      onColumnDragChange?.(null, null);
      if (!dragging && onColumnClick) {
        onColumnClick(col, ev);
      }
    }

    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
  }
</script>

<div class="ff-colh">
  {#each columns as col}
    {@const dragKey = col.key}
    {@const isDragging = columnDragKey === dragKey}
    {@const isDropTarget = columnDropKey === dragKey && columnDragKey && columnDragKey !== dragKey}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="ff-colh__cell{col.sort ? ' ff-colh__cell--active' : ''}{isDragging ? ' ff-colh__cell--dragging' : ''}{isDropTarget ? ' ff-colh__cell--drop-target' : ''}"
      style={cellStyle(col)}
      data-col-key={dragKey ?? ''}
      oncontextmenu={(e) => {
        e.preventDefault();
        onColumnContextMenu?.(e, col);
      }}
    >
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <span
        class="ff-colh__label ff-colh__label--draggable"
        onpointerdown={(e) => onLabelPointerDown(e, col)}
      >
        {col.label ?? ''}
      </span>
      {#if col.sort}
        <Icon
          name="chevron-down"
          size={10}
          strokeWidth={2.25}
          class={'ff-colh__arrow' + (col.sort === 'asc' ? ' ff-colh__arrow--asc' : '')} />
      {/if}
      {#if !col.flex}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="ff-colh__resize-handle"
          onmousedown={(e) => startResize(e, col)}
        ></div>
      {/if}
    </div>
  {/each}
</div>
