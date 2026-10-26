import { addPlaylist, defaultPlaylistName, selectVolume } from './library.svelte.ts';
import { ui, dockOrganizeTarget } from './ui.svelte.ts';
import { canDockOrganizeAt } from '../lib/newPlaylist.ts';

export async function createPlaylistAndOpen(trackIds: number[] = []): Promise<number> {
  const name = defaultPlaylistName();
  const playlistId = await addPlaylist(name, trackIds);
  if (!canDockOrganizeAt(window.innerWidth)) {
    ui.activeSidebarRow = name;
    await selectVolume(null);
  }
  dockOrganizeTarget({ kind: 'playlist', id: name, label: name, playlistId });
  ui.renameRequest = { playlistId };
  return playlistId;
}
