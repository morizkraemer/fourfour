export function canDeleteSidebarSelection(selectedIds: readonly number[], sidebarFocused: boolean): boolean {
  return sidebarFocused && selectedIds.length > 0;
}

export function isInsideSidebar(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest('[data-focus-region="sidebar"]') !== null;
}
