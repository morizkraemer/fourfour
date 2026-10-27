import { addTracksToPlaylist } from './library.svelte.ts';
import { addTracksToStickAll } from './sticks.svelte.ts';

export type CardDropTarget =
  | { kind: 'playlist'; playlistId: number }
  | { kind: 'stick'; volumePath: string };

const FLASH_MS = 1800;

export const dropFlash = $state({ key: null as string | null, text: '' });

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

function showFlash(key: string, text: string) {
  if (flashTimer) clearTimeout(flashTimer);
  dropFlash.key = key;
  dropFlash.text = text;
  flashTimer = setTimeout(() => {
    dropFlash.key = null;
    dropFlash.text = '';
  }, FLASH_MS);
}

/** Puts dragged tracks on a destination card without opening it; duplicates are skipped. */
export async function dropTracksOnCard(target: CardDropTarget, trackIds: number[]): Promise<number> {
  const requested = new Set(trackIds).size;
  const added =
    target.kind === 'playlist'
      ? await addTracksToPlaylist(target.playlistId, trackIds)
      : addTracksToStickAll(target.volumePath, trackIds);
  showFlash(cardKey(target), dropConfirmation(requested, added));
  return added;
}
