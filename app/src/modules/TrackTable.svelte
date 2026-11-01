<!--
  TrackTable.svelte — Library track table with DnD reorder, column sort,
  column visibility menu, and playlist-aware row moves.
-->
<script>
  import { onMount, tick, untrack } from 'svelte';
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
    tracksForSidebarSource,
    filterTracks,
    sidebarSourceLabel,
    isTrackAnalyzing,
    requestRowPreviews,
  } from '../stores/library.svelte.ts';
  import { commitTrackRating } from '../stores/track-edits.svelte.ts';
  import { selection, select, toggle, selectRange, setSelection, clear } from '../stores/selection.svelte.ts';
  import { player, play, queuePlayAfterLoad, playFromStart, loadPlayerTrack, loadTrackAt } from '../stores/player.svelte.ts';
  import { firstDroppedTrack } from '../lib/playerDrop.ts';
  import { ui } from '../stores/ui.svelte.ts';
  import { pickDirectory, prefetchAnalysisData, prefetchWaveformDetail } from '../services/tauri.svelte.ts';
  import { createDetailPrefetcher } from '../services/detail-prefetch.ts';
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
  import { createPlaylistAndOpen } from '../stores/new-playlist.svelte.ts';
  import { dropTracksOnCard, dropTracksOnRibbon } from '../stores/organize-drop.svelte.ts';
  import { dnd, startTrackDrag, updatePointer, updateTrackDropTarget, endDrag } from '../stores/dnd.svelte.ts';
  import {
    buildTrackContextMenuItems,
    buildColumnContextMenuItems,
    buildSourceContextMenuItems,
  } from '../menus/context-menus.ts';
  import { clear as clearSelection } from '../stores/selection.svelte.ts';
  import { fileStatus, fileStateOf } from '../stores/file-status.svelte.ts';
  import { locateTrack } from '../stores/file-health.svelte.ts';
  import { fileStateTooltip, isUnplayable } from '../lib/fileHealth.ts';
  import { rowWindow, scrollTopToReveal, indexForSlot, rowTop, spacerHeight, dropLineTop as dropLineTopFor, OVERSCAN_ROWS, ROW_HEIGHT } from '../lib/rowWindow.ts';
  import { anchorShift, sameMembers } from '../lib/stableSort.ts';
  import { bootRowTrack, bootSnapshotOf, writeBootSnapshot } from '../stores/boot-snapshot.ts';

  /** When set, show this sidebar source instead of the active row. */
  /** organize=true marks this table as the docked organize-panel destination. */
  let { sourceId = undefined, embedded = false, layoutKindOverride = undefined, organize = false } = $props();

  const uid = $props.id();
  let lastSelectedId = null;
  let cursorIndex = $state(-1);
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

  const EMPTY_TRACK = {};
  let scrollTop = $state(0);
  let viewportHeight = $state(0);
  let headerHeight = $state(0);
  let scrollRaf = null;
  const SCROLL_SETTLE_MS = 250;
  let scrolling = $state(false);
  let scrollSettleTimer = null;
  let ownScrollTop = null;

  function syncScrollWindow() {
    scrollRaf = null;
    if (!tableScrollEl) return;
    scrollTop = tableScrollEl.scrollTop;
    if (headerHeight === 0) {
      headerHeight = tableScrollEl.querySelector('.ff-colh')?.offsetHeight ?? 0;
    }
  }

  function onTableScroll() {
    if (scrollRaf == null) scrollRaf = requestAnimationFrame(syncScrollWindow);
    if (ownScrollTop !== null && tableScrollEl?.scrollTop === ownScrollTop) {
      ownScrollTop = null;
      return;
    }
    ownScrollTop = null;
    if (!scrolling) scrolling = true;
    clearTimeout(scrollSettleTimer);
    scrollSettleTimer = setTimeout(settleScroll, SCROLL_SETTLE_MS);
  }

  function settleScroll() {
    scrolling = false;
    if (resortWaiting) {
      resortWaiting = false;
      resortNonce++;
    }
  }

  onMount(() => {
    initTableColumns();
  });

  $effect(() => {
    const request = ui.revealTrack;
    if (!request || embedded) return;
    void tick().then(() => {
      if (!tableScrollEl) return;
      const index = currentTracks.findIndex((t) => t.id === request.id);
      if (index < 0) return;
      tableScrollEl.scrollTop = scrollTopToReveal(index, {
        scrollTop: tableScrollEl.scrollTop,
        viewportHeight: tableScrollEl.clientHeight,
        headerHeight,
        rowCount: currentTracks.length,
      });
    });
  });

  async function handleImport() {
    const dir = await pickDirectory();
    if (dir) {
      await importDirectory(dir, { targetSource: effectiveSource });
    }
  }

  let effectiveSource = $derived(sourceId ?? ui.activeSidebarRow);
  let showLibraryEmpty = $derived(!embedded && library.tracksLoaded && library.trackCount === 0);
  const skeletonRows = Array.from({ length: 24 }, (_, i) => i);

  let activePlaylist = $derived.by(() => {
    const active = effectiveSource;
    if (!active) return null;
    return playlistByName(active) ?? null;
  });

  let isUsbView = $derived(library.volumes.includes(effectiveSource ?? ''));

  let canReorderRows = $derived(!!activePlaylist && !isUsbView && !layout.sortKey);

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

  let showOnlyProblems = $derived(fileStatus.onlyProblems && !embedded && (effectiveSource ?? 'All Tracks') === 'All Tracks');
  let baseTracks = $derived.by(() => {
    const tracks = tracksForSidebarSource(effectiveSource);
    return showOnlyProblems ? tracks.filter((t) => isUnplayable(fileStateOf(t.id))) : tracks;
  });
  let filteredTracks = $derived(filterTracks(baseTracks, ui.listFilter));

  let layoutKind = $derived(layoutKindOverride ?? resolveLayoutKind(effectiveSource));
  let layout = $derived(getLayout(layoutKind));

  let sorted = null;
  let resortWaiting = false;
  let resortNonce = $state(0);
  let pendingAnchorPx = 0;

  let currentTracks = $derived.by(() => {
    resortNonce;
    if (!layout.sortKey) {
      sorted = null;
      return filteredTracks;
    }
    const spec = `${layoutKind}|${layout.sortKey}|${layout.sortDir}`;
    const previous = sorted;
    const sameRows = previous !== null && previous.spec === spec && sameMembers(previous.members, filteredTracks);
    if (sameRows && untrack(() => scrolling)) {
      resortWaiting = true;
      return previous.result;
    }
    const result = sortTracks(filteredTracks, layoutKind);
    sorted = { spec, members: sameRows ? previous.members : new Set(filteredTracks), result };
    if (sameRows && previous.result !== result) {
      pendingAnchorPx = untrack(() => anchorShift(previous.result, result, visibleRowRange())) * ROW_HEIGHT;
    }
    return result;
  });

  function visibleRowRange() {
    if (!tableScrollEl || tableScrollEl.scrollTop <= 0) return null;
    const top = tableScrollEl.scrollTop;
    return { first: Math.ceil(top / ROW_HEIGHT), end: Math.floor((top + tableScrollEl.clientHeight - headerHeight) / ROW_HEIGHT) };
  }

  $effect.pre(() => {
    currentTracks;
    if (pendingAnchorPx === 0 || !tableScrollEl) return;
    const shift = pendingAnchorPx;
    pendingAnchorPx = 0;
    tableScrollEl.scrollTop += shift;
    ownScrollTop = tableScrollEl.scrollTop;
    scrollTop = tableScrollEl.scrollTop;
  });

  let columns = $derived(visibleColumns(layoutKind));

  let bootView = $derived(`${layoutKind}|${layout.sortKey ?? ''}|${layout.sortDir ?? ''}`);
  let showsBootView = $derived(!embedded && (effectiveSource ?? 'All Tracks') === 'All Tracks' && !ui.listFilter.trim() && !showOnlyProblems);
  let bootRows = $derived.by(() => {
    const snapshot = library.bootSnapshot;
    if (!snapshot || library.tracksLoaded || !showsBootView || snapshot.view !== bootView) return null;
    return snapshot.rows.map(bootRowTrack);
  });
  let booting = $derived(currentTracks.length === 0 && bootRows !== null);
  let shownTracks = $derived(booting ? bootRows : currentTracks);

  const BOOT_SNAPSHOT_IDLE_MS = 1500;
  let bootSnapshotTimer;
  $effect(() => {
    if (!library.tracksLoaded || !showsBootView || viewportHeight === 0) return;
    const tracks = currentTracks;
    const path = library.libraryPath;
    const view = bootView;
    const rows = Math.ceil(viewportHeight / ROW_HEIGHT) + 2;
    clearTimeout(bootSnapshotTimer);
    bootSnapshotTimer = setTimeout(() => writeBootSnapshot(bootSnapshotOf(path, view, tracks, rows)), BOOT_SNAPSHOT_IDLE_MS);
    return () => clearTimeout(bootSnapshotTimer);
  });

  let overscan = $state(0);
  let offscreenRowsScheduled = false;
  $effect(() => {
    if (offscreenRowsScheduled || currentTracks.length === 0 || viewportHeight === 0) return;
    offscreenRowsScheduled = true;
    requestAnimationFrame(() => setTimeout(() => (overscan = OVERSCAN_ROWS)));
  });

  let rowWin = $derived(
    rowWindow({ scrollTop, viewportHeight, headerHeight, rowCount: shownTracks.length, overscan })
  );
  let firstRow = $derived(rowWin.firstRow);
  let endRow = $derived(rowWin.endRow);
  let poolSize = $derived(rowWin.poolSize);
  let poolSlots = $derived(Array.from({ length: poolSize }, (_, slot) => slot));

  $effect(() => {
    requestRowPreviews(currentTracks.slice(firstRow, endRow));
  });

  const detailPrefetcher = createDetailPrefetcher(prefetchWaveformDetail);

  function prefetchNeighbours(rowIndex) {
    const neighbours = [currentTracks[rowIndex + 1], currentTracks[rowIndex - 1]];
    prefetchAnalysisData(neighbours.filter((t) => t?.analyzed).map((t) => t.id));
    detailPrefetcher.request([currentTracks[rowIndex], currentTracks[rowIndex + 1], currentTracks[rowIndex + 2]].filter(Boolean).map((t) => t.id));
  }

  function handleRowClick(track, event, rowIndex) {
    if (suppressRowClick) {
      suppressRowClick = false;
      return;
    }
    if (dragging || marqueeActive) return;
    cursorIndex = rowIndex;
    if (event.shiftKey && lastSelectedId !== null) {
      selectRange(lastSelectedId, track.id, currentTracks);
    } else if (event.metaKey || event.ctrlKey) {
      toggle(track.id);
      lastSelectedId = track.id;
    } else {
      select(track.id);
      lastSelectedId = track.id;
      prefetchNeighbours(rowIndex);
    }
  }

  function handleRowDblClick(track, event) {
    event.preventDefault();
    openTrack(track, !!event.target.closest?.('[data-wave-seek]'));
  }

  function openTrack(track, fromWave) {
    if (dragging || isUsbView) return;
    select(track.id);
    lastSelectedId = track.id;
    if (player.track?.id === track.id && !player.loading && fromWave) {
      if (!player.playing) void play();
    } else if (player.track?.id === track.id && !player.loading) {
      void playFromStart();
    } else {
      queuePlayAfterLoad();
      void loadPlayerTrack(track);
    }
  }

  function rowId(index) {
    return `${uid}-row-${index}`;
  }

  function revealRow(index) {
    if (!tableScrollEl) return;
    const header = headerHeight || tableScrollEl.querySelector('.ff-colh')?.offsetHeight || 0;
    const top = scrollTopToReveal(index, {
      scrollTop: tableScrollEl.scrollTop,
      viewportHeight: tableScrollEl.clientHeight,
      headerHeight: header,
      rowCount: currentTracks.length,
    });
    if (top !== tableScrollEl.scrollTop) tableScrollEl.scrollTop = top;
  }

  function moveCursor(from, to, event) {
    const index = Math.min(Math.max(to, 0), currentTracks.length - 1);
    const track = currentTracks[index];
    cursorIndex = index;
    if (event.shiftKey) {
      if (lastSelectedId === null) lastSelectedId = currentTracks[Math.max(from, 0)]?.id ?? track.id;
      const anchor = Math.max(0, currentTracks.findIndex((t) => t.id === lastSelectedId));
      setSelection(currentTracks.slice(Math.min(anchor, index), Math.max(anchor, index) + 1).map((t) => t.id));
    } else {
      select(track.id);
      lastSelectedId = track.id;
      prefetchNeighbours(index);
    }
    revealRow(index);
  }

  function openRowMenuAtCursor(index) {
    const track = currentTracks[index];
    if (!track) return;
    const rect = (tableBodyEl.querySelector(`[data-row-index="${index}"]`) ?? tableBodyEl).getBoundingClientRect();
    closeTableMenus();
    rowMenu = { open: true, x: rect.left + 24, y: rect.bottom, trackId: track.id };
  }

  function onListKeydown(e) {
    if (e.target !== e.currentTarget || e.altKey || currentTracks.length === 0) return;
    const last = currentTracks.length - 1;
    const stored = cursorIndex >= 0 && cursorIndex <= last ? cursorIndex : currentTracks.findIndex((t) => selection.ids.has(t.id));
    const page = Math.max(1, Math.floor(((tableScrollEl?.clientHeight ?? 0) - headerHeight) / ROW_HEIGHT) - 1);
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a') {
      e.preventDefault();
      setSelection(currentTracks.map((t) => t.id));
      return;
    }
    if (e.ctrlKey || e.metaKey) return;
    const targets = { ArrowDown: stored + 1, ArrowUp: stored - 1, PageDown: stored + page, PageUp: stored - page, Home: 0, End: last };
    if (e.key in targets) {
      e.preventDefault();
      moveCursor(stored, targets[e.key], e);
    } else if (e.key === 'Enter' && stored >= 0) {
      e.preventDefault();
      openTrack(currentTracks[stored], false);
    } else if ((e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) && stored >= 0) {
      e.preventDefault();
      if (!selection.ids.has(currentTracks[stored].id)) select(currentTracks[stored].id);
      openRowMenuAtCursor(stored);
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
    rows.forEach((row) => {
      const rect = row.getBoundingClientRect();
      const track = currentTracks[Number(row.dataset.rowIndex)];
      if (rectsIntersect(band, rect) && track) {
        ids.push(track.id);
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
    return dropLineTopFor(index, currentTracks.length);
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
    detailPrefetcher.now(track.id);
    const target = e.target;
    if (target.closest?.('.ff-colh__resize-handle') || target.closest?.('[data-wave-seek]') || target.closest?.('[data-rate]')) return;
    e.preventDefault();

    const rowEl = e.currentTarget;
    const scrollEl = rowEl?.closest('.ff-table__scroll');
    const scrollTopAtDrag = scrollEl?.scrollTop ?? 0;
    const ids = dragTrackIds(track);
    let moved = false;
    let scrollPinRaf = null;
    let moveRaf = null;
    let lastX = e.clientX;
    let lastY = e.clientY;

    function applyPointer() {
      moveRaf = null;
      updatePointer(lastX, lastY);
      updateTrackDropTarget(lastX, lastY);
    }

    function pinSourceScroll() {
      if (!embedded || !scrollEl || scrollEl.scrollTop === scrollTopAtDrag) return;
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
      lastX = ev.clientX;
      lastY = ev.clientY;
      if (moveRaf == null) moveRaf = requestAnimationFrame(applyPointer);
      pinSourceScroll();
    }

    async function onUp(ev) {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      stopScrollPin();
      if (moveRaf != null) {
        cancelAnimationFrame(moveRaf);
        applyPointer();
      }
      scrollEl?.removeEventListener('wheel', onWheel);
      if (rowEl?.hasPointerCapture?.(e.pointerId)) {
        rowEl.releasePointerCapture(e.pointerId);
      }
      if (!moved) return;

      const dropNewPlaylist = dnd.dropNewPlaylist;
      const dropPlId = dnd.dropPlaylistId;
      const dropIdx = dnd.dropRowIndex;
      const dropSource = dnd.dropPanelSourceId;
      const copyOnly = ev.altKey;
      const cardPlaylistId = dnd.dropCardPlaylistId;
      const cardStickPath = dnd.dropStickPath;
      const ribbonDrop = dnd.dropRibbon;

      if (dnd.dropPlayer) {
        const dropped = firstDroppedTrack(ids, library.tracks);
        if (dropped) void loadPlayerTrack(dropped);
        dragging = false;
        dropLineTop = null;
        endDrag();
        return;
      }

      if (ribbonDrop) {
        await dropTracksOnRibbon(ui.organize.target, ids);
        dragging = false;
        dropLineTop = null;
        endDrag();
        return;
      }

      if (cardPlaylistId != null || cardStickPath != null) {
        await dropTracksOnCard(
          cardPlaylistId != null
            ? { kind: 'playlist', playlistId: cardPlaylistId }
            : { kind: 'stick', volumePath: cardStickPath },
          ids
        );
        dragging = false;
        dropLineTop = null;
        endDrag();
        return;
      }

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
          await dropTracksOnCard({ kind: 'stick', volumePath: organizeTarget.id }, ids);
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

      if (dropNewPlaylist) {
        await createPlaylistAndOpen(ids);
      } else if (dropPlId != null) {
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
          } else if (!samePanel) {
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
    const items = buildTrackContextMenuItems({
      trackIds: ids,
      activePlaylist,
      isUsbView,
      onTracksRemoved: () => clearSelection(),
    });
    if (ids.length !== 1 || isUsbView || !isUnplayable(fileStateOf(trackId))) return items;
    return [{ label: 'Locate file…', onClick: () => void locateTrack(trackId) }, { isSeparator: true }, ...items];
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
      class:ff-table__scroll--scrolling={scrolling}
      bind:this={tableScrollEl}
      bind:clientHeight={viewportHeight}
      onscroll={onTableScroll}
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
        onColumnAutoFit={(key) => fitColumnToContent(layoutKind, key, tableScrollEl, currentTracks)}
      />
      <div
        class="ff-table__body"
        class:ff-table__body--marquee={marqueeActive}
        data-track-drop-body
        data-source-id={effectiveSource}
        data-row-count={currentTracks.length}
        data-organize-drop={organize ? 'true' : undefined}
        bind:this={tableBodyEl}
        style:height={spacerHeight(shownTracks.length) !== undefined ? `${spacerHeight(shownTracks.length)}px` : undefined}
        data-table-boot={booting ? '' : undefined}
        aria-busy={booting ? 'true' : undefined}
        inert={booting}
        role="listbox"
        tabindex="0"
        data-focus-first
        aria-label="Tracks"
        aria-multiselectable="true"
        aria-activedescendant={cursorIndex >= firstRow && cursorIndex < endRow && !booting ? rowId(cursorIndex) : undefined}
        onkeydown={onListKeydown}
        ondragover={(e) => e.preventDefault()}
        onpointerdown={onBodyPointerDown}
        onselectstart={(e) => e.preventDefault()}
      >
      {#if isDropTarget && dropLineTop != null}
        <div class="ff-table__drop-line" style:top="{dropLineTop}px">
          <DropLine />
        </div>
      {/if}
      {#if shownTracks.length === 0 && !library.tracksLoaded}
        <div class="ff-table__skeleton" data-table-skeleton aria-busy="true">
          {#each skeletonRows as row (row)}
            <div class="ff-table__skeleton-row" style:--skeleton-text="{55 + ((row * 17) % 40)}%">
              <TrackRow track={EMPTY_TRACK} {columns} />
            </div>
          {/each}
        </div>
      {:else if shownTracks.length === 0}
        <div class="ff-table__empty-container ff-table__empty-drop">
          <EmptyState
            title="No tracks in this view"
            sub={activePlaylist
              ? 'Drag tracks here to add them to this playlist.'
              : 'Select a playlist or import music.'}
          />
        </div>
      {:else}
        {#each poolSlots as slot (slot)}
          {@const index = indexForSlot(slot, firstRow, poolSize)}
          {@const track = index < endRow ? shownTracks[index] : undefined}
          {@const fileState = track && !isUsbView ? fileStateOf(track.id) : undefined}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="ff-table__row"
            data-track-row={track && !booting ? '' : undefined}
            data-boot-row={track && booting ? '' : undefined}
            data-row-index={index}
            data-cursor={index === cursorIndex ? '' : undefined}
            id={rowId(index)}
            role={track ? 'option' : undefined}
            aria-selected={track ? selection.ids.has(track.id) : undefined}
            aria-posinset={track ? index + 1 : undefined}
            aria-setsize={track ? shownTracks.length : undefined}
            data-file-status={fileState}
            title={fileState ? fileStateTooltip(fileState, track.filePath) : undefined}
            style:top="{rowTop(index)}px"
            style:display={track ? undefined : 'none'}
            onclick={(e) => track && handleRowClick(track, e, index)}
            ondblclick={(e) => track && handleRowDblClick(track, e)}
            onpointerdown={(e) => track && onRowPointerDown(e, track, index)}
            oncontextmenu={(e) => track && openRowMenu(e, track)}
          >
            <TrackRow
              track={track ?? EMPTY_TRACK}
              state={track ? rowState(track) : 'rest'}
              analyzing={track ? isTrackAnalyzing(track.id) : false}
              {columns}
              onWaveSeek={track && !isUsbView && !booting ? (progress) => loadTrackAt(track, progress) : undefined}
              onRate={track && !isUsbView && !booting ? (stars) => void commitTrackRating(track.id, stars) : undefined}
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
  .ff-table__scroll--scrolling :global(.ff-waveform--analysing) {
    animation: none;
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
  .ff-table__body:focus {
    outline: none;
  }
  .ff-table__body:focus-visible .ff-table__row[data-cursor] {
    outline: 1px solid var(--ff-accent);
    outline-offset: -1px;
  }
  .ff-table {
    -webkit-user-select: none;
    user-select: none;
  }
  .ff-table__row {
    position: absolute;
    left: 0;
    right: 0;
    height: 28px;
    contain: layout paint style;
    cursor: default;
  }
  .ff-table__row :global(.ff-trow__cell--status) {
    justify-content: center;
    padding: 0;
  }
  .ff-table__row[data-file-status] :global(.ff-trow__cell--status .ff-trow__text) {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: 0 0 16px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    font: 700 10px / 1 var(--ff-font-mono);
  }
  .ff-table__row[data-file-status='missing'] :global(.ff-trow__cell--status .ff-trow__text) {
    background: color-mix(in srgb, var(--ff-status-warn) 16%, transparent);
    color: var(--ff-status-warn);
  }
  .ff-table__row[data-file-status='error'] :global(.ff-trow__cell--status .ff-trow__text) {
    background: var(--ff-danger-bg);
    color: var(--ff-danger);
  }
  .ff-table__row[data-file-status='relocated'] :global(.ff-trow__cell--status .ff-trow__text) {
    background: color-mix(in srgb, var(--ff-status-online) 14%, transparent);
    color: var(--ff-status-online);
  }
  .ff-table__row[data-file-status='missing'] :global(.ff-trow__cell:not(.ff-trow__cell--status)),
  .ff-table__row[data-file-status='error'] :global(.ff-trow__cell:not(.ff-trow__cell--status)) {
    opacity: 0.45;
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
  .ff-table__skeleton {
    animation: ff-table-skeleton-pulse 1.4s ease-in-out infinite;
    pointer-events: none;
  }
  .ff-table__skeleton-row {
    height: 28px;
  }
  .ff-table__skeleton-row :global(.ff-trow:hover) {
    background: none;
  }
  .ff-table__skeleton-row :global(.ff-trow__cell--text::before),
  .ff-table__skeleton-row :global(.ff-trow__cell--wave::before) {
    content: '';
    display: block;
    flex: none;
    height: 8px;
    border-radius: 3px;
    background: var(--ff-elev);
  }
  .ff-table__skeleton-row :global(.ff-trow__cell--text::before) {
    width: var(--skeleton-text);
    max-width: 100%;
  }
  .ff-table__skeleton-row :global(.ff-trow__cell--index::before),
  .ff-table__skeleton-row :global(.ff-trow__cell--bpm::before),
  .ff-table__skeleton-row :global(.ff-trow__cell--time::before),
  .ff-table__skeleton-row :global(.ff-trow__cell--key::before) {
    width: 70%;
  }
  .ff-table__skeleton-row :global(.ff-trow__cell--text .ff-trow__text),
  .ff-table__skeleton-row :global(.ff-trow__cell--wave > *),
  .ff-table__skeleton-row :global(.ff-trow__star) {
    display: none;
  }
  .ff-table__skeleton-row :global(.ff-trow__cell--wave::before) {
    width: 100%;
    height: 14px;
  }
  .ff-table__skeleton-row :global(.ff-trow__cover) {
    border-color: transparent;
  }
  @keyframes ff-table-skeleton-pulse {
    0%, 100% { opacity: 0.55; }
    50% { opacity: 1; }
  }
  .ff-table__empty-drop:global(.ff-table__empty-drop) {
    min-height: 120px;
  }
  .ff-table--drop-target {
    outline: 1px solid var(--ff-accent);
    outline-offset: -1px;
  }
</style>
