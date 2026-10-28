import { library } from './library.svelte.ts';
import { ui } from './ui.svelte.ts';
import { sidebarSelection } from './sidebar-selection.svelte.ts';
import { canDeleteSidebarSelection, isInsideSidebar } from '../lib/sidebarDelete.ts';
import { confirmDeletePlaylists } from '../menus/context-menus.ts';

const sidebarFocus = { inside: false };

if (typeof window !== 'undefined') {
  const track = (e: Event) => {
    sidebarFocus.inside = isInsideSidebar(e.target);
  };
  window.addEventListener('pointerdown', track, true);
  window.addEventListener('focusin', track, true);
}

function isNonPlaylistRow(id: string): boolean {
  return (
    id === 'All Tracks' ||
    id === 'Recently Added' ||
    library.volumes.includes(id) ||
    library.sidebarData.favorites.rows.some((row) => row.label === id)
  );
}

export function resetActiveRowIfDeleted(): void {
  const stillThere = library.playlists.some((p) => p.name === ui.activeSidebarRow && !p.is_folder);
  if (!stillThere && !isNonPlaylistRow(ui.activeSidebarRow)) ui.activeSidebarRow = 'All Tracks';
}

export function sidebarDeleteReady(): boolean {
  return canDeleteSidebarSelection(sidebarSelection.ids, sidebarFocus.inside);
}

export async function deleteSidebarSelection(): Promise<void> {
  if (!sidebarDeleteReady()) return;
  if (await confirmDeletePlaylists([...sidebarSelection.ids])) resetActiveRowIfDeleted();
}
