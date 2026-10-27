import { addTracksToPlaylist, library } from './library.svelte.ts';
import { addPlaylistsToStick, addTracksToStickAll } from './sticks.svelte.ts';
import { expandOrganize, type OrganizeTarget } from './ui.svelte.ts';

export type CardDropTarget =
  | { kind: 'playlist'; playlistId: number }
  | { kind: 'stick'; volumePath: string };

const FLASH_MS = 1800;

export const dropFlash = $state({
  key: null as string | null,
  text: '',
  added: 0,
  cover: null as string | null,
});

let flashTimer: ReturnType<typeof setTimeout> | null = null;

export function cardKey(target: CardDropTarget): string {
  return target.kind === 'playlist' ? `playlist:${target.playlistId}` : `stick:${target.volumePath}`;
}

export function dropConfirmation(requested: number, added: number): string {
  if (added === 0) return requested === 1 ? 'Already there' : 'All already there';
  const addedText = `Added ${added}`;
  const skipped = requested - added;
  return skipped > 0 ? `${addedText} · ${skipped} already there` : addedText;
}

function showFlash(key: string, text: string, added: number, cover: string | null) {
  if (flashTimer) clearTimeout(flashTimer);
  dropFlash.key = key;
  dropFlash.text = text;
  dropFlash.added = added;
  dropFlash.cover = cover;
  flashTimer = setTimeout(() => {
    dropFlash.key = null;
    dropFlash.text = '';
    dropFlash.added = 0;
    dropFlash.cover = null;
  }, FLASH_MS);
}

function firstCover(trackIds: number[]): string | null {
  const byId = new Map(library.tracks.map((t) => [t.id, t]));
  for (const id of trackIds) {
    const cover = byId.get(id)?.cover;
    if (cover) return cover;
  }
  return null;
}

/** Puts dragged tracks on a destination card without opening it; duplicates are skipped. */
export async function dropTracksOnCard(target: CardDropTarget, trackIds: number[]): Promise<number> {
  const requested = new Set(trackIds).size;
  const added =
    target.kind === 'playlist'
      ? await addTracksToPlaylist(target.playlistId, trackIds)
      : await addTracksToStickAll(target.volumePath, trackIds);
  showFlash(cardKey(target), dropConfirmation(requested, added), added, added > 0 ? firstCover(trackIds) : null);
  return added;
}

export function playlistDropConfirmation(added: number): string {
  return added > 0 ? 'Playlist on stick' : 'Already on stick';
}

/** Puts a playlist on a stick; only playlists put on a stick are written to it. */
export async function dropPlaylistOnStick(volumePath: string, playlistId: number): Promise<number> {
  const added = await addPlaylistsToStick(volumePath, [playlistId]);
  const playlist = library.playlists.find((p) => p.id === playlistId);
  const cover = added > 0 && playlist ? firstCover(playlist.track_ids) : null;
  showFlash(cardKey({ kind: 'stick', volumePath }), playlistDropConfirmation(added), added, cover);
  return added;
}

export function boardDropTarget(board: OrganizeTarget): CardDropTarget | null {
  if (board.kind === 'usb') return { kind: 'stick', volumePath: board.id };
  return typeof board.playlistId === 'number' ? { kind: 'playlist', playlistId: board.playlistId } : null;
}

export type RibbonDropOutcome = 'added' | 'opened';

/** Tracks dropped on the collapsed ribbon join the open board; with no board the drawer opens to the picker. */
export async function dropTracksOnRibbon(
  board: OrganizeTarget | null,
  trackIds: number[]
): Promise<RibbonDropOutcome> {
  const target = board && boardDropTarget(board);
  if (!target) {
    expandOrganize();
    return 'opened';
  }
  await dropTracksOnCard(target, trackIds);
  return 'added';
}
