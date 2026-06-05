<!--
  OrganizePanel.svelte — Persistent right-edge organize panel.

  Visual states:
    rail     (ui.organize.expanded === false): thin sliver at the right edge.
              Clicking or dragging right expands to the panel.
    expanded (ui.organize.expanded === true):  a panel of ui.organize.width px.
              Left-edge drag handle resizes with magnetic snap:
                - Below ORGANIZE_SNAP_TO_RAIL_PX → snap to rail
                - Within ORGANIZE_SNAP_TO_FULL_PX of the viewport edge → snap to full width

  Panel body (s2):
    - target === null → <OrganizePicker />
    - target !== null → <TrackTable sourceId={target.id} embedded />
      Header shows target.label; inline rename when renaming=true.
-->
<script>
  import { Button, Icon } from '$ds';
  import {
    ui,
    expandOrganize,
    collapseOrganize,
    closeOrganize,
    setOrganizeWidth,
    commitOrganizeWidth,
    ORGANIZE_SNAP_TO_RAIL_PX,
    ORGANIZE_SNAP_TO_FULL_PX,
  } from '../stores/ui.svelte.ts';
  import { renamePlaylist, tracksForSidebarSource } from '../stores/library.svelte.ts';
  import OrganizePicker from './OrganizePicker.svelte';
  import TrackTable from './TrackTable.svelte';

  // The host (.ff-browse__content) is the resize container.
  // We receive a ref to it via prop so we can read its bounding rect.
  let { contentEl = null } = $props();

  // ── Inline rename for the docked target label ─────────────────────────────
  // When a new playlist is created and docked, the header enters rename mode
  // automatically. The OrganizePicker calls dockOrganizeTarget, which sets
  // target; we detect a freshly-created playlist by listening for the
  // renamePending flag exposed by OrganizePicker via a callback prop.

  let renamingHeader = $state(false);
  let renameDraft = $state('');
  let renameHeaderEl = $state(null);

  /** Called by OrganizePicker after it creates+docks a new playlist. */
  function startHeaderRename(name) {
    renamingHeader = true;
    renameDraft = name;
    // Focus on next tick after DOM updates
    setTimeout(() => renameHeaderEl?.select(), 0);
  }

  async function commitHeaderRename() {
    if (!renamingHeader) return;
    renamingHeader = false;
    const draft = renameDraft.trim();
    const target = ui.organize.target;
    if (!target || !draft || draft === target.label) return;
    if (target.kind === 'playlist' && typeof target.playlistId === 'number') {
      await renamePlaylist(target.playlistId, draft);
      // Update the reactive target in ui store
      ui.organize.target = { ...target, id: draft, label: draft };
    }
  }

  function cancelHeaderRename() {
    renamingHeader = false;
  }

  // ── Filled-rail cover glance (s8) ─────────────────────────────────────────

  /**
   * Up to 4 cover thumbnails from the docked target, shown when the rail is
   * collapsed and a target is docked. `track.cover` is a blob URL populated by
   * loadState() → getTrackArtwork(). We take the first MAX_RAIL_COVERS tracks
   * that have a cover URL; if fewer exist we show the ones we have.
   */
  const MAX_RAIL_COVERS = 4;

  const railCovers = $derived.by(() => {
    const target = ui.organize.target;
    if (!target) return [];
    const tracks = tracksForSidebarSource(target.id);
    const result = [];
    for (const t of tracks) {
      if (t.cover) {
        result.push(t.cover);
        if (result.length >= MAX_RAIL_COVERS) break;
      }
    }
    return result;
  });

  const railTrackCount = $derived.by(() => {
    const target = ui.organize.target;
    if (!target) return 0;
    return tracksForSidebarSource(target.id).length;
  });

  // ── Rail drag-to-expand ────────────────────────────────────────────────────

  /**
   * Rail interaction is split into two clean paths:
   *   - click  → expand to last width (onRailClick)
   *   - drag   → resize live from the rail (onRailPointerDown)
   *
   * We deliberately do NOT use setPointerCapture: expanding mid-drag unmounts
   * the rail element, and a captured pointer fires `pointercancel` the moment
   * its target leaves the DOM — which would kill the gesture. Window-level
   * listeners survive the rail→panel swap, so we rely on those instead.
   */
  let suppressNextClick = false;

  function onRailClick() {
    // A click that immediately follows a drag is swallowed (the drag already
    // grew the panel — re-expanding would fight it).
    if (suppressNextClick) {
      suppressNextClick = false;
      return;
    }
    expandOrganize();
  }

  /**
   * Pulling the rail out is a one-way gesture: as soon as it qualifies as a
   * drag we commit to expanded and let the width follow the cursor. We must NOT
   * route through applyResizeWidth here — at the rail the candidate width is
   * tiny (≈10px), which is below the snap-to-rail threshold, so it would
   * collapse on every move and the panel would never open. Collapsing back to
   * the rail is the job of the expanded panel's own resize handle.
   */
  function onRailPointerDown(e) {
    if (e.button !== 0) return;

    const startX = e.clientX;
    let dragged = false;

    const container = contentEl ?? document.querySelector('.ff-browse__content');
    const rect = container ? container.getBoundingClientRect() : null;
    const containerRight = rect ? rect.right : window.innerWidth;
    const containerLeft = rect ? rect.left : 0;
    const maxWidth = containerRight - containerLeft;

    function onMove(ev) {
      if (!dragged) {
        if (startX - ev.clientX <= 6) return; // only leftward drags pull it out
        dragged = true;
        expandOrganize();
      }
      let w = containerRight - ev.clientX;
      if (w > maxWidth - ORGANIZE_SNAP_TO_FULL_PX) w = maxWidth; // snap to full near the edge
      setOrganizeWidth(w); // clamps to the configured min width
    }

    function onUp() {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      window.removeEventListener('pointercancel', onUp);
      if (dragged) {
        suppressNextClick = true;
        commitOrganizeWidth();
      }
    }

    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    window.addEventListener('pointercancel', onUp);
  }

  // ── Panel left-edge resize handle ─────────────────────────────────────────

  function onResizeHandlePointerDown(e) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    e.currentTarget.setPointerCapture?.(e.pointerId);
    document.body.classList.add('ff-organize--resizing');

    function onMove(ev) {
      const container = contentEl ?? document.querySelector('.ff-browse__content');
      const containerRight = container
        ? container.getBoundingClientRect().right
        : window.innerWidth;
      const newWidth = containerRight - ev.clientX;
      applyResizeWidth(newWidth, containerRight);
    }

    function onUp() {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      window.removeEventListener('pointercancel', onUp);
      document.body.classList.remove('ff-organize--resizing');
      commitOrganizeWidth();
    }

    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    window.addEventListener('pointercancel', onUp);
  }

  /**
   * Apply a candidate width with magnetic snap logic.
   * Snap thresholds are intentionally tunable via the exported constants
   * in ui.svelte.ts (ORGANIZE_SNAP_TO_RAIL_PX, ORGANIZE_SNAP_TO_FULL_PX).
   */
  function applyResizeWidth(rawWidth, containerRight) {
    // Snap to rail (collapse) when width drops below threshold
    if (rawWidth < ORGANIZE_SNAP_TO_RAIL_PX) {
      collapseOrganize();
      return;
    }

    // Snap to full width when close to the container's right edge
    const container = contentEl ?? document.querySelector('.ff-browse__content');
    const containerLeft = container ? container.getBoundingClientRect().left : 0;
    const maxWidth = containerRight - containerLeft;
    if (rawWidth > maxWidth - ORGANIZE_SNAP_TO_FULL_PX) {
      setOrganizeWidth(maxWidth);
      return;
    }

    setOrganizeWidth(rawWidth);
    // Ensure we stay expanded if we're setting a valid width
    if (!ui.organize.expanded) expandOrganize();
  }
</script>

{#if ui.organize.expanded}
  <!-- ── Expanded panel ─────────────────────────────────────────────────── -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="ff-organize ff-organize--expanded"
    style:width="{ui.organize.width}px"
  >
    <!-- Left-edge resize handle -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="ff-organize__resize-handle"
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize organize panel"
      onpointerdown={onResizeHandlePointerDown}
    ></div>

    <!-- Panel header -->
    <div class="ff-organize__header">
      {#if renamingHeader && ui.organize.target}
        <!-- Inline rename input for newly-created playlists -->
        <!-- svelte-ignore a11y_autofocus -->
        <input
          bind:this={renameHeaderEl}
          class="ff-organize__header-rename"
          type="text"
          bind:value={renameDraft}
          onkeydown={(e) => {
            if (e.key === 'Enter') { e.preventDefault(); void commitHeaderRename(); }
            if (e.key === 'Escape') cancelHeaderRename();
          }}
          onblur={commitHeaderRename}
        />
      {:else}
        <span class="ff-organize__header-title">
          {ui.organize.target ? ui.organize.target.label : 'Organize'}
        </span>
      {/if}
      <div class="ff-organize__header-actions">
        <!-- Collapse to rail -->
        <Button
          icon="chevron-right"
          iconOnly
          variant="ghost"
          size="small"
          ariaLabel="Collapse to rail"
          onclick={collapseOrganize}
        />
        <!-- Close / clear -->
        <Button
          icon="x"
          iconOnly
          variant="ghost"
          size="small"
          ariaLabel="Close organize panel"
          onclick={closeOrganize}
        />
      </div>
    </div>

    <!-- Panel body -->
    <div class="ff-organize__body" class:ff-organize__body--table={ui.organize.target !== null}>
      {#if ui.organize.target === null}
        <!-- No target: show destination picker -->
        <OrganizePicker onNewPlaylistDocked={startHeaderRename} />
      {:else}
        <!-- Target docked: show its tracks -->
        <TrackTable sourceId={ui.organize.target.id} embedded layoutKindOverride="organize" organize={true} />
      {/if}
    </div>
  </div>
{:else}
  <!-- ── Rail sliver ──────────────────────────────────────────────────────── -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="ff-organize ff-organize--rail{ui.organize.target ? ' ff-organize--rail-filled' : ' ff-organize--rail-empty'}"
    role="button"
    tabindex="0"
    aria-label={ui.organize.target ? `Open ${ui.organize.target.label} (${railTrackCount} tracks)` : 'Open organize panel'}
    onclick={onRailClick}
    onpointerdown={onRailPointerDown}
    onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && expandOrganize()}
  >
    {#if ui.organize.target}
      <!-- Filled rail: cover thumbnails + track count -->
      <div class="ff-organize__rail-glance" aria-hidden="true">
        <div class="ff-organize__rail-covers">
          {#each railCovers as coverUrl}
            <div
              class="ff-organize__rail-cover"
              style:background-image="url({coverUrl})"
            ></div>
          {/each}
          {#if railCovers.length === 0}
            <!-- No covers available: show a placeholder block -->
            <div class="ff-organize__rail-cover ff-organize__rail-cover--placeholder"></div>
          {/if}
        </div>
        <span class="ff-organize__rail-count">{railTrackCount}</span>
      </div>
    {:else}
      <!-- Empty rail (Spotify-style): a chevron toggle floating on the edge. -->
      <div class="ff-organize__rail-chevron" aria-hidden="true">
        <Icon name="chevron-left" size={16} />
      </div>
    {/if}
  </div>
{/if}

<style>
  /* ── Shared ────────────────────────────────────────────────────────────── */
  .ff-organize {
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100%;
    flex-shrink: 0;
    background: var(--ff-surface);
    border-left: 1px solid var(--ff-border);
  }

  /* ── Rail ──────────────────────────────────────────────────────────────── */
  .ff-organize--rail {
    width: 20px;
    cursor: col-resize;
    align-items: center;
    justify-content: center;
    touch-action: none;
    user-select: none;
    transition: background 0.1s ease;
  }

  /* Filled-collapsed rail: a Spotify-style column of cover thumbnails */
  .ff-organize--rail-filled {
    width: 60px;
  }

  .ff-organize--rail:hover {
    background: var(--ff-hover);
  }

  /* ── Empty rail (Spotify-style): thin edge + chevron toggle ─────────────── */
  .ff-organize--rail-empty {
    width: 14px;
    background: transparent;
    border-left: 1px solid var(--ff-border);
    overflow: visible;          /* let the chevron button straddle the edge */
  }

  .ff-organize--rail-empty:hover {
    background: var(--ff-hover);
  }

  .ff-organize__rail-chevron {
    position: absolute;
    left: -11px;                /* sit astride the panel edge */
    top: 50%;
    transform: translateY(-50%);
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--ff-elev);
    border: 1px solid var(--ff-border-hi);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--ff-text-mid);
    transition: background 0.12s ease, color 0.12s ease, border-color 0.12s ease;
  }

  .ff-organize--rail-empty:hover .ff-organize__rail-chevron {
    background: var(--ff-elev-hi);
    border-color: var(--ff-border-hover);
    color: var(--ff-text);
  }

  /* ── Filled-rail glance (s8) ─────────────────────────────────────────── */
  .ff-organize__rail-glance {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--ff-space-2, 4px);
    padding: var(--ff-space-3, 6px) 0;
    width: 100%;
  }

  .ff-organize__rail-covers {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    width: 44px;
  }

  .ff-organize__rail-cover {
    width: 44px;
    height: 44px;
    border-radius: 6px;
    background-color: var(--ff-surface-hi, var(--ff-surface));
    background-size: cover;
    background-position: center;
    flex-shrink: 0;
  }

  .ff-organize__rail-cover--placeholder {
    background-color: var(--ff-surface-hi, var(--ff-surface));
    opacity: 0.4;
  }

  .ff-organize__rail-count {
    font-size: 10px;
    font-weight: 600;
    color: var(--ff-text-dim, var(--ff-faint));
    letter-spacing: 0.02em;
    line-height: 1;
  }

  .ff-organize--rail-filled:hover .ff-organize__rail-count {
    color: var(--ff-text, currentColor);
  }

  /* ── Expanded panel ────────────────────────────────────────────────────── */
  .ff-organize--expanded {
    min-width: 0;
  }

  /* Left-edge drag handle */
  .ff-organize__resize-handle {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 5px;
    cursor: col-resize;
    touch-action: none;
    z-index: 4;
  }

  .ff-organize__resize-handle::after {
    content: '';
    position: absolute;
    left: 2px;
    top: 0;
    bottom: 0;
    width: 1px;
    background: var(--ff-border-hi);
    opacity: 0.5;
    transition: opacity 0.12s ease, background 0.12s ease;
  }

  .ff-organize__resize-handle:hover::after {
    opacity: 1;
    background: var(--ff-border-hover, var(--ff-border-hi));
  }

  :global(body.ff-organize--resizing) .ff-organize__resize-handle::after {
    opacity: 1;
    background: var(--ff-accent);
  }

  :global(body.ff-organize--resizing) {
    cursor: col-resize !important;
    user-select: none;
  }

  :global(body.ff-organize--resizing) * {
    cursor: col-resize !important;
  }

  /* Header */
  .ff-organize__header {
    display: flex;
    align-items: center;
    height: 40px;
    padding: 0 var(--ff-space-3) 0 var(--ff-space-5);
    gap: var(--ff-space-2);
    border-bottom: 1px solid var(--ff-border);
    flex-shrink: 0;
  }

  .ff-organize__header-title {
    flex: 1 1 0;
    font-size: var(--ff-text-sm, 12px);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--ff-text-dim, var(--ff-faint));
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Inline rename input in header */
  .ff-organize__header-rename {
    flex: 1 1 0;
    background: var(--ff-input-bg, var(--ff-surface-hi));
    border: 1px solid var(--ff-accent);
    border-radius: var(--ff-radius-sm, 4px);
    color: var(--ff-text);
    font-size: var(--ff-text-sm, 12px);
    font-weight: 600;
    padding: 2px var(--ff-space-2);
    outline: none;
    min-width: 0;
  }

  .ff-organize__header-actions {
    display: flex;
    align-items: center;
    gap: var(--ff-space-1);
    flex-shrink: 0;
  }

  /* Body */
  .ff-organize__body {
    flex: 1 1 0;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--ff-space-5);
  }

  /* When a target is docked the body holds a TrackTable — remove centering padding */
  .ff-organize__body--table {
    padding: 0;
    align-items: stretch;
    justify-content: stretch;
  }

  .ff-organize__body--table > :global(*) {
    flex: 1 1 0;
    min-width: 0;
    min-height: 0;
  }
</style>
