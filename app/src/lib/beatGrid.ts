export interface BeatMarker {
  time_ms: number;
  bar_position: number;
}

export const BEAT_NUDGE_MS = 10;
export const BEAT_NUDGE_FINE_MS = 1;

/**
 * One nudge: one CSS pixel of the zoom view, between 1 ms and BEAT_NUDGE_MS; `fine` (⌥) is always 1 ms.
 * Beat times are whole milliseconds (the library and the PQTZ beat grid of the export store ms), so 1 ms is the floor.
 */
export function gridNudgeMs(msPerPixel: number, fine: boolean): number {
  if (fine) return BEAT_NUDGE_FINE_MS;
  if (!(msPerPixel > 0)) return BEAT_NUDGE_MS;
  return Math.min(BEAT_NUDGE_MS, Math.max(BEAT_NUDGE_FINE_MS, Math.round(msPerPixel)));
}

/** The grid moved by `deltaMs`; beats that would start before the track are dropped (a stored beat time is never negative). */
export function shiftBeats(beats: BeatMarker[], deltaMs: number): BeatMarker[] {
  const delta = Math.round(deltaMs);
  const out: BeatMarker[] = [];
  for (const beat of beats) {
    const time = beat.time_ms + delta;
    if (time >= 0) out.push({ time_ms: time, bar_position: beat.bar_position });
  }
  return out;
}

/** How far to move the grid so the downbeat nearest to `positionMs` lands exactly on it. */
export function shiftToPlaceDownbeatAt(beats: BeatMarker[], positionMs: number): number | null {
  if (!beats.length || !Number.isFinite(positionMs)) return null;
  const downbeats = beats.filter((b) => b.bar_position === 1);
  const pool = downbeats.length ? downbeats : beats;
  let nearest = pool[0].time_ms;
  for (const beat of pool) {
    if (Math.abs(beat.time_ms - positionMs) < Math.abs(nearest - positionMs)) nearest = beat.time_ms;
  }
  return Math.round(positionMs - nearest);
}

export { SAVE_DELAY_MS as GRID_SAVE_DELAY_MS, createPersister as createGridPersister } from './persister.ts';
export type { Persister as GridPersister } from './persister.ts';
