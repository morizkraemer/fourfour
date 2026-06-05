<!--
  OrganizePanel.svelte — Persistent right-edge organize panel (s1 shell).

  Visual states:
    rail     (ui.organize.expanded === false): thin sliver at the right edge.
              Clicking or dragging right expands to the panel.
    expanded (ui.organize.expanded === true):  a panel of ui.organize.width px.
              Left-edge drag handle resizes with magnetic snap:
                - Below ORGANIZE_SNAP_TO_RAIL_PX → snap to rail
                - Within ORGANIZE_SNAP_TO_FULL_PX of the viewport edge → snap to full width

  s1 panel body is empty — the picker + table arrive in s2.
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

  // The host (.ff-browse__content) is the resize container.
  // We receive a ref to it via prop so we can read its bounding rect.
  let { contentEl = null } = $props();

  // ── Rail drag-to-expand ────────────────────────────────────────────────────

  /**
   * Dragging on the rail itself: if the pointer moves far enough left (into the
   * content area), expand the panel and set its width based on drag position.
   * A simple click on the rail also expands.
   */
  function onRailPointerDown(e) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.currentTarget.setPointerCapture?.(e.pointerId);

    const startX = e.clientX;
    let dragged = false;

    function onMove(ev) {
      const delta = startX - ev.clientX; // positive = dragging left (expanding)
      if (!dragged && delta > 6) {
        dragged = true;
        expandOrganize();
      }
      if (dragged) {
        const container = contentEl ?? document.querySelector('.ff-browse__content');
        const containerRight = container
          ? container.getBoundingClientRect().right
          : window.innerWidth;
        const newWidth = containerRight - ev.clientX;
        applyResizeWidth(newWidth, containerRight);
      }
    }

    function onUp() {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      window.removeEventListener('pointercancel', onUp);
      commitOrganizeWidth();
      if (!dragged) {
        // Simple click: expand to last width
        expandOrganize();
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
      <span class="ff-organize__header-title">
        {ui.organize.target ? ui.organize.target.label : 'Organize'}
      </span>
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

    <!-- Panel body — empty in s1; picker + table arrive in s2 -->
    <div class="ff-organize__body">
      <div class="ff-organize__empty-hint">
        <Icon name="list" size={20} />
        <span>Pick a destination to organize tracks.</span>
      </div>
    </div>
  </div>
{:else}
  <!-- ── Rail sliver ──────────────────────────────────────────────────────── -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="ff-organize ff-organize--rail"
    role="button"
    tabindex="0"
    aria-label="Open organize panel"
    onpointerdown={onRailPointerDown}
    onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && expandOrganize()}
  >
    <div class="ff-organize__rail-handle" aria-hidden="true">
      <Icon name="chevron-left" size={12} />
    </div>
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

  .ff-organize--rail:hover {
    background: var(--ff-hover);
  }

  .ff-organize__rail-handle {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--ff-faint);
    transition: color 0.1s ease;
  }

  .ff-organize--rail:hover .ff-organize__rail-handle {
    color: var(--ff-text-dim, var(--ff-faint));
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

  .ff-organize__empty-hint {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--ff-space-3);
    color: var(--ff-faint);
    font-size: var(--ff-text-sm, 12px);
    text-align: center;
  }
</style>
