<script>
  // Column header row (Svelte port of column-header.js). Driven by the same
  // `columns` array as TrackRow so widths stay aligned.
  //   columns: [{ label, width?, flex?, align?, sort?:'asc'|'desc', key? }]
  import './column-header.css';
  import Icon from './Icon.svelte';
  import { columnCellStyle } from '../utils/column-layout.js';

  let {
    columns = [],
    columnDragKey = null,
    columnDropKey = null,
    onColumnContextMenu,
    onColumnClick,
    onColumnReorder,
    onColumnDragChange,
    onColumnResize,
  } = $props();

  let resizeCol = null;
  let resizeCell = null;
  let resizeStartX = 0;
  let resizeStartWidth = 0;

  function startResize(e, col) {
    if (!col.key || e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    const cell = e.currentTarget.closest('.ff-colh__cell');
    if (!cell) return;

    resizeCol = col;
    resizeCell = cell;
    resizeStartX = e.clientX;
    resizeStartWidth = cell.getBoundingClientRect().width;

    cell.setPointerCapture?.(e.pointerId);
    window.addEventListener('pointermove', handleResize);
    window.addEventListener('pointerup', stopResize);
    window.addEventListener('pointercancel', stopResize);
    document.body.classList.add('ff-colh--resizing');
  }

  function handleResize(e) {
    if (!resizeCol || !resizeCell) return;
    const deltaX = e.clientX - resizeStartX;
    const next = Math.round(resizeStartWidth + deltaX);
    if (resizeCol.flex) {
      resizeCol.minWidth = next;
    } else {
      resizeCol.width = next;
    }
  }

  function stopResize(e) {
    if (resizeCol?.key && resizeCell) {
      const width = resizeCell.getBoundingClientRect().width;
      onColumnResize?.(resizeCol.key, width);
    }
    if (resizeCell && e?.pointerId != null) {
      try {
        resizeCell.releasePointerCapture(e.pointerId);
      } catch {
        /* already released */
      }
    }
    resizeCol = null;
    resizeCell = null;
    window.removeEventListener('pointermove', handleResize);
    window.removeEventListener('pointerup', stopResize);
    window.removeEventListener('pointercancel', stopResize);
    document.body.classList.remove('ff-colh--resizing');
  }

  function onLabelPointerDown(e, col) {
    if (!col.key || e.button !== 0) return;
    if (e.target.closest?.('.ff-colh__resize-handle')) return;
    e.preventDefault();
    const startKey = col.key;
    let dragging = false;
    let dropKey = null;

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
        dropKey = hoverKey;
        onColumnDragChange?.(startKey, hoverKey);
      }
    }

    function onUp(ev) {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      if (dragging && dropKey && dropKey !== startKey) {
        onColumnReorder?.(startKey, dropKey);
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
      style={columnCellStyle(col)}
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
      {#if col.key}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="ff-colh__resize-handle"
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize {col.label || col.key} column"
          onpointerdown={(e) => startResize(e, col)}
        ></div>
      {/if}
    </div>
  {/each}
</div>
