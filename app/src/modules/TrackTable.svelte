<!--
  TrackTable.svelte — Library track table with DnD reorder, column sort,
  column visibility menu, and playlist-aware row moves.
-->
<script>
  import { onMount } from 'svelte';
  import {
    ColumnHeader,
    TrackRow,
    EmptyState,
    Button,
    ContextMenu,
    DragGhost,
    DropLine,
  } from '$ds';
  import {
    library,
    importDirectory,
    playlistByName,
    reorderPlaylistTracks,
    moveTracksToPlaylist,
    exportTracksToVolume,
    tracksForSidebarSource,
    filterTracks,
    sidebarSourceLabel,
    isTrackAnalyzing,
  } from '../stores/library.svelte.ts';
  import { selection, select, toggle, selectRange, setSelection, clear } from '../stores/selection.svelte.ts';
  import { player, queuePlayAfterLoad, playFromStart, loadPlayerTrack } from '../stores/player.svelte.ts';
  import { ui } from '../stores/ui.svelte.ts';
  import { pickDirectory } from '../services/tauri.svelte.ts';
  import {
    getLayout,
    resolveLayoutKind,
    visibleColumns,
    initTableColumns,
    moveColumn,
    toggleColumnVisibility,
    resetColumns,
    setColumnWidth,
    fitColumnToContent,
    setSort,
    clearSort,
    sortTracks,
    ALL_COLUMN_KEYS,
  } from '../stores/table-columns.svelte.ts';
  import { dnd, startTrackDrag, updatePointer, updateTrackDropTarget, endDrag } from '../stores/dnd.svelte.ts';
  import {
    buildTrackContextMenuItems,
    buildColumnContextMenuItems,
    buildSourceContextMenuItems,
  } from '../menus/context-menus.ts';
  import { clear as clearSelection } from '../stores/selection.svelte.ts';

  /** When set, show this sidebar source instead of the active row. */
  /** organize=true marks this table as the docked organize-panel destination. */
  let { sourceId = undefined, embedded = false, layoutKindOverride = undefined, organize = false } = $props();

  let lastSelectedId = null;
  let tableBodyEl = $state(null);
  let columnMenu = $state({ open: false, x: 0, y: 0 });
  let rowMenu = $state({ open: false, x: 0, y: 0, trackId: null });
  let listMenu = $state({ open: false, x: 0, y: 0 });
  let colDragKey = $state(null);
  let colDropKey = $state(null);
  let tableScrollEl = $state(null);
  let dragging = $state(false);
  let dropLineTop = $state(null);
  let marqueeActive = $state(false);
  let suppressRowClick = false;

  onMount(() => {
    initTableColumns();
  });

  async function handleImport() {
    const dir = await pickDirectory();
    if (dir) {
      await importDirectory(dir, { targetSource: effectiveSource });
    }
  }

  let effectiveSource = $derived(sourceId ?? ui.activeSidebarRow);
  let showLibraryEmpty = $derived(!embedded && library.trackCount === 0);

  let activePlaylist = $derived.by(() => {
    const active = effectiveSource;
    if (!active) return null;
    return playlistByName(active) ?? null;
  });

  let isUsbView = $derived(library.volumes.includes(effectiveSource ?? ''));

  let canReorderRows = $derived(!!activePlaylist && !isUsbView);

  // Organize USB target: accept drops when this table is the docked organize
  // destination and it is a mounted USB volume.
  let isOrganizeUsbDropTarget = $derived(
    organize &&
      dnd.kind === 'tracks' &&
      dnd.dropPanelSourceId === effectiveSource &&
      isUsbView &&
      ui.organize.target?.kind === 'usb' &&
      ui.organize.target?.id === effectiveSource
  );

  let isDropTarget = $derived(
    isOrganizeUsbDropTarget ||
    (
      dnd.kind === 'tracks' &&
      dnd.dropPanelSourceId === effectiveSource &&
      playlistByName(effectiveSource ?? '') != null &&
      !isUsbView
    )
  );

  let baseTracks = $derived(tracksForSidebarSource(effectiveSource));
  let filteredTracks = $derived(filterTracks(baseTracks, ui.listFilter));

  let layoutKind = $derived(layoutKindOverride ?? resolveLayoutKind(effectiveSource));
  let layout = $derived(getLayout(layoutKind));

  let currentTracks = $derived.by(() => {
    if (canReorderRows) return filteredTracks;
    return layout.sortKey ? sortTracks(filteredTracks, layoutKind) : filteredTracks;
  });

  let columns = $derived(visibleColumns(layoutKind));

  function handleRowClick(track, event) {
    if (suppressRowClick) {
      suppressRowClick = false;
      return;
    }
    if (dragging || marqueeActive) return;
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

  function handleRowDblClick(track, event) {
    event.preventDefault();
    if (dragging || isUsbView) return;
    select(track.id);
    lastSelectedId = track.id;
    if (player.track?.id === track.id && !player.loading) {
      void playFromStart();
    } else {
      queuePlayAfterLoad();
      void loadPlayerTrack(track);
    }
  }

  function rectsIntersect(a, b) {
    return a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top;
  }

  function idsInMarqueeRect(x1, y1, x2, y2) {
    if (!tableBodyEl) return [];
    const left = Math.min(x1, x2);
    const right = Math.max(x1, x2);
    const top = Math.min(y1, y2);
    const bottom = Math.max(y1, y2);
    const band = { left, right, top, bottom };
    const ids = [];
    const rows = tableBodyEl.querySelectorAll('[data-track-row]');
    rows.forEach((row, i) => {
      const rect = row.getBoundingClientRect();
      if (rectsIntersect(band, rect) && currentTracks[i]) {
        ids.push(currentTracks[i].id);
      }
    });
    return ids;
  }

  function onBodyPointerDown(e) {
    if (e.button !== 0) return;
    if (e.target.closest('[data-track-row]')) return;

    const startX = e.clientX;
    const startY = e.clientY;
    let moved = false;

    function onMove(ev) {
      if (!moved && (Math.abs(ev.clientX - startX) > 4 || Math.abs(ev.clientY - startY) > 4)) {
        moved = true;
        marqueeActive = true;
      }
      if (!moved) return;
      ev.preventDefault();
      const ids = idsInMarqueeRect(startX, startY, ev.clientX, ev.clientY);
      if (ev.metaKey || ev.ctrlKey) {
        const next = new Set(selection.ids);
        for (const id of ids) next.add(id);
        setSelection(next);
      } else {
        setSelection(ids);
      }
    }

    function onUp() {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      if (moved) {
        suppressRowClick = true;
      } else if (!(e.metaKey || e.ctrlKey || e.shiftKey)) {
        clear();
      }
      marqueeActive = false;
    }

    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
  }

  function rowState(track) {
    if (dnd.kind === 'tracks' && dnd.trackIds.includes(track.id)) return 'drag';
    if (selection.ids.has(track.id)) return 'selected';
    return 'rest';
  }

  function dragTrackIds(track) {
    if (selection.ids.has(track.id) && selection.ids.size > 0) {
      return [...selection.ids];
    }
    return [track.id];
  }

  function ghostLabelFor(ids) {
    if (ids.length > 1) return `${ids.length} tracks`;
    const t = library.tracks.find(tr => tr.id === ids[0]);
    if (!t) return '1 track';
    return `${t.artist} — ${t.title}`;
  }

  function dropLineTopForIndex(index) {
    if (!tableBodyEl || index == null) return null;
    const rows = tableBodyEl.querySelectorAll('[data-track-row]');
    if (rows.length === 0) return 0;
    if (index >= rows.length) {
      const last = rows[rows.length - 1];
      return last.offsetTop + last.offsetHeight;
    }
    return rows[index].offsetTop;
  }

  $effect(() => {
    if (!isDropTarget || dnd.dropRowIndex == null || !tableBodyEl) {
      dropLineTop = null;
      return;
    }
    dropLineTop = dropLineTopForIndex(dnd.dropRowIndex);
  });

  function onRowPointerDown(e, track, index) {
    if (isUsbView || e.button !== 0) return;
    const target = e.target;
    if (target.closest?.('.ff-colh__resize-handle')) return;
    e.preventDefault();

    const rowEl = e.currentTarget;
    const scrollEl = rowEl?.closest('.ff-table__scroll');
    const scrollTopAtDrag = scrollEl?.scrollTop ?? 0;
    const ids = dragTrackIds(track);
    let moved = false;
    let scrollPinRaf = null;

    function pinSourceScroll() {
      if (!embedded || !scrollEl) return;
      scrollEl.scrollTop = scrollTopAtDrag;
    }

    function onWheel(ev) {
      if (!moved) return;
      ev.preventDefault();
    }

    function startScrollPin() {
      const tick = () => {
        pinSourceScroll();
        scrollPinRaf = requestAnimationFrame(tick);
      };
      scrollPinRaf = requestAnimationFrame(tick);
    }

    function stopScrollPin() {
      if (scrollPinRaf != null) {
        cancelAnimationFrame(scrollPinRaf);
        scrollPinRaf = null;
      }
    }

    function onMove(ev) {
      if (embedded) {
        ev.preventDefault();
        pinSourceScroll();
      }
      if (!moved && (Math.abs(ev.clientX - e.clientX) > 5 || Math.abs(ev.clientY - e.clientY) > 5)) {
        moved = true;
        dragging = true;
        rowEl?.setPointerCapture?.(e.pointerId);
        startTrackDrag(
          ids,
          ghostLabelFor(ids),
          activePlaylist?.id ?? null,
          effectiveSource,
          ev.clientX,
          ev.clientY
        );
        if (embedded) {
          startScrollPin();
          scrollEl?.addEventListener('wheel', onWheel, { passive: false });
        }
      }
      if (!moved) return;
      ev.preventDefault();
      updatePointer(ev.clientX, ev.clientY);
      updateTrackDropTarget(ev.clientX, ev.clientY);
      pinSourceScroll();
    }

    async function onUp(ev) {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      stopScrollPin();
      scrollEl?.removeEventListener('wheel', onWheel);
      if (rowEl?.hasPointerCapture?.(e.pointerId)) {
        rowEl.releasePointerCapture(e.pointerId);
      }
      if (!moved) return;

      const dropPlId = dnd.dropPlaylistId;
      const dropIdx = dnd.dropRowIndex;
      const dropSource = dnd.dropPanelSourceId;
      const copyOnly = ev.altKey;

      // ── Organize-panel drop routing ──────────────────────────────────────
      // If the drop landed on the docked organize panel, route to
      // playlist-add@index (playlist target) or USB export (usb target).
      // This runs before the generic branch so USB drops aren't silently skipped.
      const organizeTarget = ui.organize.target;
      if (
        dropSource != null &&
        organizeTarget != null &&
        dropSource === organizeTarget.id &&
        dropSource !== effectiveSource // only cross-panel drops (not same-panel reorder)
      ) {
        if (organizeTarget.kind === 'playlist' && typeof organizeTarget.playlistId === 'number') {
          // Add dragged tracks to the docked playlist at the drop index
          await moveTracksToPlaylist({
            trackIds: ids,
            toPlaylistId: organizeTarget.playlistId,
            sourcePlaylistId: dnd.sourcePlaylistId,
            insertIndex: dropIdx ?? undefined,
            removeFromSource: !copyOnly && dnd.sourcePlaylistId != null,
          });
        } else if (organizeTarget.kind === 'usb') {
          // Trigger export to the docked USB volume (full sync — see exportTracksToVolume)
          await exportTracksToVolume(ids, organizeTarget.id);
        } else {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          console.error(
            '[organize] Drop onto unsupported target kind — no action taken.',
            organizeTarget
          );
        }
        dragging = false;
        dropLineTop = null;
        endDrag();
        return;
      }

      if (dropPlId != null) {
        await moveTracksToPlaylist({
          trackIds: ids,
          toPlaylistId: dropPlId,
          sourcePlaylistId: dnd.sourcePlaylistId,
          removeFromSource: !copyOnly,
        });
      } else if (dropSource != null && dropIdx != null) {
        const targetPlaylist = playlistByName(dropSource);
        if (targetPlaylist && !library.volumes.includes(dropSource)) {
          const samePanel = dropSource === effectiveSource;
          if (samePanel && canReorderRows && activePlaylist) {
            const ordered = currentTracks.map(t => t.id);
            const without = ordered.filter(id => !ids.includes(id));
            const insertAt = Math.min(Math.max(0, dropIdx), without.length);
            const newOrder = [...without.slice(0, insertAt), ...ids, ...without.slice(insertAt)];
            await reorderPlaylistTracks(activePlaylist.id, newOrder);
          } else {
            await moveTracksToPlaylist({
              trackIds: ids,
              toPlaylistId: targetPlaylist.id,
              sourcePlaylistId: dnd.sourcePlaylistId,
              insertIndex: dropIdx,
              removeFromSource: !copyOnly && dnd.sourcePlaylistId != null,
            });
          }
        }
      }

      dragging = false;
      dropLineTop = null;
      endDrag();
    }

    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
  }

  function closeTableMenus() {
    columnMenu = { ...columnMenu, open: false };
    rowMenu = { ...rowMenu, open: false, trackId: null };
    listMenu = { ...listMenu, open: false };
  }

  function openColumnMenu(e) {
    closeTableMenus();
    columnMenu = { open: true, x: e.clientX, y: e.clientY };
  }

  function columnMenuItems() {
    return buildColumnContextMenuItems({
      allColumnKeys: ALL_COLUMN_KEYS,
      layout,
      toggleColumnVisibility: (key) => toggleColumnVisibility(layoutKind, key),
      resetColumns: () => resetColumns(layoutKind),
      clearSort: () => clearSort(layoutKind),
    });
  }

  function openRowMenu(e, track) {
    e.preventDefault();
    closeTableMenus();
    rowMenu = { open: true, x: e.clientX, y: e.clientY, trackId: track.id };
  }

  function openListMenu(e) {
    if (e.target.closest('[data-track-row]') || e.target.closest('.ff-colh')) return;
    e.preventDefault();
    closeTableMenus();
    listMenu = { open: true, x: e.clientX, y: e.clientY };
  }

  function listMenuItems() {
    const source = effectiveSource ?? 'All Tracks';
    return buildSourceContextMenuItems({
      sourceId: source,
      label: sidebarSourceLabel(source),
    });
  }

  function rowMenuItems() {
    const trackId = rowMenu.trackId;
    if (trackId == null) return [];
    const ids = selection.ids.has(trackId) ? [...selection.ids] : [trackId];
    return buildTrackContextMenuItems({
      trackIds: ids,
      activePlaylist,
      isUsbView,
      onTracksRemoved: () => clearSelection(),
    });
  }

</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="ff-table"
  class:ff-table--embedded={embedded}
  class:ff-table--drop-target={isDropTarget}
  data-source-id={effectiveSource}
  onmousedown={() => {}}
  oncontextmenu={openListMenu}
>
  {#if showLibraryEmpty}
    <div class="ff-table__empty-container">
      <EmptyState
        title="No tracks loaded"
        sub="Import a folder or drop audio files anywhere in this window."
      >
        <Button
          label="Import"
          icon="folder-open"
          variant="primary"
          size="small"
          disabled={library.analyzing || library.syncing}
          onclick={handleImport}
        />
      </EmptyState>
    </div>
  {:else}
    <div
      class="ff-table__scroll"
      class:ff-table__scroll--dragging={dragging && embedded}
      bind:this={tableScrollEl}
    >
      <ColumnHeader
        {columns}
        columnDragKey={colDragKey}
        columnDropKey={colDropKey}
        onColumnContextMenu={(e) => openColumnMenu(e)}
        onColumnClick={(col) => {
          if (col.key && col.key !== 'cover' && col.key !== 'wave') {
            setSort(layoutKind, col.key);
          }
        }}
        onColumnReorder={(from, to, insertBefore) => moveColumn(layoutKind, from, to, insertBefore)}
        onColumnDragChange={(drag, drop) => {
          colDragKey = drag;
          colDropKey = drop;
        }}
        onColumnResize={(key, width, save = true) => setColumnWidth(layoutKind, key, width, save)}
        onColumnAutoFit={(key) => fitColumnToContent(layoutKind, key, tableScrollEl)}
      />
      <div
        class="ff-table__body"
        class:ff-table__body--marquee={marqueeActive}
        data-track-drop-body
        data-source-id={effectiveSource}
        data-organize-drop={organize ? 'true' : undefined}
        bind:this={tableBodyEl}
        role="list"
        ondragover={(e) => e.preventDefault()}
        onpointerdown={onBodyPointerDown}
        onselectstart={(e) => e.preventDefault()}
      >
      {#if isDropTarget && dropLineTop != null}
        <div class="ff-table__drop-line" style:top="{dropLineTop}px">
          <DropLine />
        </div>
      {/if}
      {#if currentTracks.length === 0}
        <div class="ff-table__empty-container ff-table__empty-drop">
          <EmptyState
            title="No tracks in this view"
            sub={activePlaylist
              ? 'Drag tracks here to add them to this playlist.'
              : 'Select a playlist or import music.'}
          />
        </div>
      {:else}
        {#each currentTracks as track, i (track.id)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            data-track-row
            onclick={(e) => handleRowClick(track, e)}
            ondblclick={(e) => handleRowDblClick(track, e)}
            onpointerdown={(e) => onRowPointerDown(e, track, i)}
            oncontextmenu={(e) => openRowMenu(e, track)}
          >
            <TrackRow
              {track}
              state={rowState(track)}
              analyzing={isTrackAnalyzing(track.id)}
              {columns}
            />
          </div>
        {/each}
      {/if}
      </div>
    </div>
  {/if}
</div>

<ContextMenu
  open={columnMenu.open}
  x={columnMenu.x}
  y={columnMenu.y}
  items={columnMenuItems()}
  onClose={() => (columnMenu = { ...columnMenu, open: false })}
/>

<ContextMenu
  open={rowMenu.open}
  x={rowMenu.x}
  y={rowMenu.y}
  items={rowMenuItems()}
  onClose={() => (rowMenu = { ...rowMenu, open: false, trackId: null })}
/>

<ContextMenu
  open={listMenu.open}
  x={listMenu.x}
  y={listMenu.y}
  items={listMenuItems()}
  onClose={() => (listMenu = { ...listMenu, open: false })}
/>

<DragGhost
  visible={dnd.kind === 'tracks'}
  label={dnd.ghostLabel}
  count={dnd.ghostCount}
  x={dnd.pointerX}
  y={dnd.pointerY}
/>

<style>
  .ff-table {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    background: var(--ff-bg);
  }
  .ff-table__scroll {
    flex: 1 1 0;
    min-height: 0;
    min-width: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
  }
  .ff-table__scroll--dragging {
    overflow: hidden;
    touch-action: none;
  }
  .ff-table__scroll :global(.ff-colh) {
    flex-shrink: 0;
    position: sticky;
    top: 0;
    z-index: 2;
    background: var(--ff-bg);
  }
  .ff-table__body {
    position: relative;
    flex: 1 0 auto;
    display: flex;
    flex-direction: column;
    min-width: 0;
    width: 100%;
  }
  .ff-table__drop-line {
    position: absolute;
    left: 0;
    right: 0;
    height: 0;
    pointer-events: none;
    z-index: 1;
  }
  .ff-table__body--marquee {
    cursor: default;
  }
  .ff-table {
    -webkit-user-select: none;
    user-select: none;
  }
  .ff-table__body > :global(div[data-track-row]) {
    cursor: default;
  }
  .ff-table__empty-container {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    padding: var(--ff-space-8);
    box-sizing: border-box;
  }
  .ff-table__empty-container :global(.ff-empty-state) {
    width: 100%;
    max-width: 480px;
  }
  .ff-table__empty-drop:global(.ff-table__empty-drop) {
    min-height: 120px;
  }
  .ff-table--drop-target {
    outline: 1px solid var(--ff-accent);
    outline-offset: -1px;
  }
</style>
