import { addTracksToPlaylist, library } from './library.svelte.ts';
import { addPlaylistsToStick, addTracksToStickAll, stickName, stickPlaylistIds } from './sticks.svelte.ts';
import { dnd } from './dnd.svelte.ts';
import { playlistIdsInSelection } from '../lib/playlistSelection.ts';
import { stickDropLabel } from '../lib/stickDrop.ts';
import { expandOrganize, type OrganizeTarget } from './ui.svelte.ts';
import { artwork } from './artwork.svelte.ts';
import { trackIdsWithCover } from '../lib/coverTracks.ts';

export type CardDropTarget =
  | { kind: 'playlist'; playlistId: number }
  | { kind: 'stick'; volumePath: string };

const FLASH_MS = 1800;

export const dropFlash = $state({
  key: null as string | null,
  text: '',
  added: 0,
  cover: null as string | null,
  coverTrackId: null as number | null,
});

artwork.keep('drop-flash', () => (dropFlash.coverTrackId === null ? [] : [dropFlash.coverTrackId]));

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

interface FlashCover {
  trackId: number;
  url: string;
}

function showFlash(key: string, text: string, added: number, cover: FlashCover | null) {
  if (flashTimer) clearTimeout(flashTimer);
  dropFlash.key = key;
  dropFlash.text = text;
  dropFlash.added = added;
  dropFlash.cover = cover?.url ?? null;
  dropFlash.coverTrackId = cover?.trackId ?? null;
  flashTimer = setTimeout(() => {
    dropFlash.key = null;
    dropFlash.text = '';
    dropFlash.added = 0;
    dropFlash.cover = null;
    dropFlash.coverTrackId = null;
  }, FLASH_MS);
}

const FLASH_COVER_CANDIDATES = 4;

function firstCover(trackIds: number[]): FlashCover | null {
  const byId = new Map(library.tracks.map((t) => [t.id, t]));
  const tracks = trackIds.flatMap((id) => byId.get(id) ?? []);
  for (const trackId of trackIdsWithCover(tracks, FLASH_COVER_CANDIDATES)) {
    const url = artwork.get(trackId, true);
    if (url) return { trackId, url };
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

export function playlistDropConfirmation(added: number, copied = false): string {
  if (added === 0) return 'Already on stick';
  return copied ? 'Copied · Keep updated' : 'Playlist on stick';
}

/** Puts playlists on a stick, following the library ("Keep updated"); also how a playlist is copied from another stick. */
export async function dropPlaylistsOnStick(volumePath: string, playlistIds: number[], copied = false): Promise<number> {
  const added = await addPlaylistsToStick(volumePath, playlistIds);
  const playlist = library.playlists.find((p) => p.id === playlistIds[0]);
  const cover = added > 0 && playlist ? firstCover(playlist.track_ids) : null;
  showFlash(cardKey({ kind: 'stick', volumePath }), playlistDropConfirmation(added, copied), added, cover);
  return added;
}

/** What dropping the current drag on the hovered stick would do, in one line; empty when no stick is hovered. */
export function hoveredStickDropLabel(): string {
  const path = dnd.dropStickPath;
  if (!path) return '';
  if (dnd.kind === 'tracks') return stickDropLabel(stickName(path), { kind: 'tracks', count: dnd.trackIds.length });
  if (dnd.kind !== 'playlist') return '';
  const ids = playlistIdsInSelection(library.playlists, dnd.playlistDragIds);
  const onStick = stickPlaylistIds(path);
  return stickDropLabel(stickName(path), {
    kind: 'playlist',
    fromStick: dnd.playlistDragFromStick !== null,
    alreadyThere: ids.length > 0 && ids.every((id) => onStick.includes(id)),
  });
}

export function dropPlaylistOnStick(volumePath: string, playlistId: number): Promise<number> {
  return dropPlaylistsOnStick(volumePath, [playlistId]);
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
