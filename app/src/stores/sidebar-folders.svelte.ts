const COLLAPSED_FOLDERS_KEY = 'ff.sidebar.collapsedFolders';

function loadCollapsed(): number[] {
  try {
    const parsed = JSON.parse(localStorage.getItem(COLLAPSED_FOLDERS_KEY) ?? '[]');
    return Array.isArray(parsed) ? parsed.filter((id): id is number => typeof id === 'number') : [];
  } catch {
    return [];
  }
}

function persistCollapsed() {
  try {
    localStorage.setItem(COLLAPSED_FOLDERS_KEY, JSON.stringify(folderState.collapsed));
  } catch {
    return;
  }
}

export const folderState = $state({ collapsed: loadCollapsed() });

export function toggleFolder(id: number) {
  folderState.collapsed = folderState.collapsed.includes(id)
    ? folderState.collapsed.filter((x) => x !== id)
    : [...folderState.collapsed, id];
  persistCollapsed();
}

export function expandFolder(id: number) {
  if (!folderState.collapsed.includes(id)) return;
  folderState.collapsed = folderState.collapsed.filter((x) => x !== id);
  persistCollapsed();
}
