/**
 * ui.svelte.ts — UI panel state for the Browse screen.
 * State lives in a single exported object so consumers can read + write fields.
 */

// ── Organize panel ──────────────────────────────────────────────────────────

export type OrganizeTarget = { kind: 'playlist' | 'usb'; id: string; label: string };

const ORGANIZE_WIDTH_KEY = 'fourfour.organize.width';
const ORGANIZE_EXPANDED_KEY = 'fourfour.organize.expanded';

/**
 * Default panel width (px) when pulled out. Tune here.
 * Min panel width before snapping to rail: ORGANIZE_SNAP_TO_RAIL_PX (below).
 * Snap-to-fullscreen threshold: ORGANIZE_SNAP_TO_FULL_PX (below).
 */
const ORGANIZE_WIDTH_DEFAULT = 360;
const ORGANIZE_WIDTH_MIN = 200; // absolute floor; below SNAP_RAIL threshold snaps to rail
const ORGANIZE_WIDTH_MAX = 99999; // effectively unlimited (clamped by viewport at render)

/** Magnetic snap thresholds — tunable. */
export const ORGANIZE_SNAP_TO_RAIL_PX = 120;   // if dragged width < this → snap to rail
export const ORGANIZE_SNAP_TO_FULL_PX = 120;   // if gap to viewport right < this → snap to full

export const ui = $state({
  /** Whether the right detail pane is visible. */
  detailPaneOpen: true,
  /** Whether the left sidebar is collapsed (future). */
  sidebarCollapsed: false,
  /** Which sidebar row is currently active (drives library view). */
  activeSidebarRow: 'All Tracks',
  /** Preferences modal opened from the footer settings button. */
  settingsOpen: false,
  /** ⌘F inline filter — narrows visible track lists in place. */
  listFilter: '',
  /** Incremented to focus the filter field (GlobalHeader). */
  filterFocusNonce: 0,
  /** Right-sidebar organize panel state. */
  organize: {
    /** Docked target — null in s1; s2 fills it. */
    target: null as OrganizeTarget | null,
    /** false = rail only; true = pulled-out panel. */
    expanded: false as boolean,
    /** Panel width in px when expanded. */
    width: ORGANIZE_WIDTH_DEFAULT as number,
  },
});

// ── Organize init / persist ──────────────────────────────────────────────────

export function initOrganize() {
  try {
    const rawW = localStorage.getItem(ORGANIZE_WIDTH_KEY);
    if (rawW) {
      const n = parseFloat(rawW);
      if (Number.isFinite(n)) ui.organize.width = clampOrganizeWidth(n);
    }
    const rawE = localStorage.getItem(ORGANIZE_EXPANDED_KEY);
    if (rawE === 'true') ui.organize.expanded = true;
  } catch {
    /* ignore */
  }
}

function persistOrganize() {
  try {
    localStorage.setItem(ORGANIZE_WIDTH_KEY, String(ui.organize.width));
    localStorage.setItem(ORGANIZE_EXPANDED_KEY, String(ui.organize.expanded));
  } catch {
    /* ignore */
  }
}

function clampOrganizeWidth(w: number): number {
  return Math.max(ORGANIZE_WIDTH_MIN, Math.min(ORGANIZE_WIDTH_MAX, w));
}

/** Pull the organize panel out to its last width. */
export function expandOrganize() {
  ui.organize.expanded = true;
  persistOrganize();
}

/** Collapse to rail — keeps the target intact. */
export function collapseOrganize() {
  ui.organize.expanded = false;
  persistOrganize();
}

/** Close the organize panel: clear target and collapse. */
export function closeOrganize() {
  ui.organize.target = null;
  ui.organize.expanded = false;
  persistOrganize();
}

/** Set panel width (clamped). Call commitOrganizeWidth() after drag ends to persist. */
export function setOrganizeWidth(px: number) {
  ui.organize.width = clampOrganizeWidth(px);
}

/** Persist the current width — call on pointer-up after a resize drag. */
export function commitOrganizeWidth() {
  persistOrganize();
}

// ── Other UI state ────────────────────────────────────────────────────────────

/** Clear the inline list filter. */
export function clearListFilter() {
  ui.listFilter = '';
}

/** Focus the global filter input (⌘F). */
export function focusListFilter() {
  ui.filterFocusNonce += 1;
}

/** Toggle the detail pane. */
export function toggleDetailPane() {
  ui.detailPaneOpen = !ui.detailPaneOpen;
}

export function openSettings() {
  ui.settingsOpen = true;
}

export function closeSettings() {
  ui.settingsOpen = false;
}

/** Sidebar source id used when importing via drag-and-drop. */
export function importDropTarget(): string {
  return ui.activeSidebarRow;
}
