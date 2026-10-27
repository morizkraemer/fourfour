import { loadPlayerTrack, player, seekToProgress } from './player.svelte.ts';

/** Seek the track to `progress` (0..1); a track that is not in the player is loaded first. */
export async function seekTrackToProgress(track: { id: number }, progress: number): Promise<void> {
  if (player.track?.id !== track.id) await loadPlayerTrack(track);
  await seekToProgress(progress);
}

/** Jump the track to `timeMs`; a track that is not in the player is loaded first. */
export function seekTrackToMs(track: { id: number; raw?: { duration_secs?: number } }, timeMs: number): Promise<void> {
  const durationMs = (track.raw?.duration_secs ?? 0) * 1000;
  return durationMs > 0 ? seekTrackToProgress(track, timeMs / durationMs) : Promise.resolve();
}
