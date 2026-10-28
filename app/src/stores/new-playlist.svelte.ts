import { addPlaylist, defaultPlaylistName, selectVolume } from './library.svelte.ts';
import { ui, dockOrganizeTarget } from './ui.svelte.ts';
import { canDockOrganizeAt } from '../lib/newPlaylist.ts';

export async function createPlaylistAndOpen(
  trackIds: number[] = [],
  parentId = 0,
  name = defaultPlaylistName()
): Promise<number> {
  const playlistId = await addPlaylist(name, trackIds, parentId);
  if (!canDockOrganizeAt(window.innerWidth)) {
    ui.activeSidebarRow = name;
    await selectVolume(null);
  }
  dockOrganizeTarget({ kind: 'playlist', id: name, label: name, playlistId });
  ui.sidebarRenameRequest = { playlistId };
  return playlistId;
}
