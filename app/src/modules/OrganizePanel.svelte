<!--
  OrganizePanel.svelte — Right sidebar: a full-height column at the window edge,
  outside the main view. Shown or hidden by the top bar toggle (Shift-Tab).
  Left-edge drag handle resizes; dragging below ORGANIZE_SNAP_TO_RAIL_PX hides it.

  Body:
    - target === null → <OrganizePicker />
    - target !== null → <TrackTable sourceId={target.id} embedded />
      Header shows target.label (inline rename) and a Change button.
-->
<script>
  import { Button, ContextMenu } from '$ds';
  import {
    ui,
    expandOrganize,
    collapseOrganize,
    clearOrganizeTarget,
    setOrganizeWidth,
    commitOrganizeWidth,
    ORGANIZE_SNAP_TO_RAIL_PX,
    ORGANIZE_SNAP_TO_FULL_PX,
  } from '../stores/ui.svelte.ts';
  import { renamePlaylist } from '../stores/library.svelte.ts';
  import { buildSourceContextMenuItems } from '../menus/context-menus.ts';
  import OrganizePicker from './OrganizePicker.svelte';
  import TrackTable from './TrackTable.svelte';

  let { containerEl = null } = $props();

  let renamingHeader = $state(false);
  let renameDraft = $state('');
  let renameHeaderEl = $state(null);
  let headerMenu = $state({ open: false, x: 0, y: 0 });

  function openHeaderMenu(e) {
    if (ui.organize.target?.kind !== 'playlist') return;
    e.preventDefault();
    headerMenu = { open: true, x: e.clientX, y: e.clientY };
  }

  function headerMenuItems() {
    const target = ui.organize.target;
    if (!target) return [];
    return buildSourceContextMenuItems({
      sourceId: target.id,
      label: target.label,
      onRenamePlaylist: (playlist) => startHeaderRename(playlist.name),
    });
  }

  $effect(() => {
    const request = ui.renameRequest;
    if (!request) return;
    ui.renameRequest = null;
    if (ui.organize.target?.playlistId === request.playlistId) {
      startHeaderRename(ui.organize.target.label);
    }
  });

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

  // ── Panel left-edge resize handle ─────────────────────────────────────────

  function onResizeHandlePointerDown(e) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    e.currentTarget.setPointerCapture?.(e.pointerId);
    document.body.classList.add('ff-organize--resizing');

    function onMove(ev) {
      const containerRight = containerEl
        ? containerEl.getBoundingClientRect().right
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
   * Snap thresholds are tunable via the exported constants
   * in ui.svelte.ts (ORGANIZE_SNAP_TO_RAIL_PX, ORGANIZE_SNAP_TO_FULL_PX).
   */
  function applyResizeWidth(rawWidth, containerRight) {
    // Hide the sidebar when width drops below threshold
    if (rawWidth < ORGANIZE_SNAP_TO_RAIL_PX) {
      collapseOrganize();
      return;
    }

    // Snap to full width when close to the container's right edge
    const containerLeft = containerEl ? containerEl.getBoundingClientRect().left : 0;
    const maxWidth = containerRight - containerLeft - ui.sidebarWidth;
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
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="ff-organize__header" oncontextmenu={openHeaderMenu}>
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
      {#if ui.organize.target}
        <div class="ff-organize__header-actions">
          <Button label="Change" variant="default" size="small" onclick={clearOrganizeTarget} />
        </div>
      {/if}
    </div>

    <!-- Panel body -->
    <div class="ff-organize__body" class:ff-organize__body--table={ui.organize.target !== null}>
      {#if ui.organize.target === null}
        <!-- No target: show destination picker -->
        <OrganizePicker />
      {:else}
        <!-- Target docked: show its tracks -->
        <TrackTable sourceId={ui.organize.target.id} embedded layoutKindOverride="organize" organize={true} />
      {/if}
    </div>
  </div>
{/if}

<ContextMenu
  open={headerMenu.open}
  x={headerMenu.x}
  y={headerMenu.y}
  items={headerMenuItems()}
  onClose={() => (headerMenu = { ...headerMenu, open: false })}
/>

<style>
  .ff-organize {
    position: relative;
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
    height: 100%;
    background: var(--ff-surface);
    border-left: 1px solid var(--ff-border-hi);
    overflow: hidden;
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
    opacity: 0;
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
    height: var(--ff-titlebar-band-height);
    padding: 0 var(--ff-space-4) 0 var(--ff-space-5);
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
