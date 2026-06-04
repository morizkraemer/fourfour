/**
 * ui.svelte.ts — UI panel state for the Browse screen.
 * State lives in a single exported object so consumers can read + write fields.
 */

export const ui = $state({
  /** Whether the right detail pane is visible. */
  detailPaneOpen: true,
  /** Whether the left sidebar is collapsed (future). */
  sidebarCollapsed: false,
  /** Which sidebar row is currently active (drives library view). */
  activeSidebarRow: 'all-tracks',
});

/** Toggle the detail pane. */
export function toggleDetailPane() {
  ui.detailPaneOpen = !ui.detailPaneOpen;
}
