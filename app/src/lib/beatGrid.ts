export interface BeatMarker {
  time_ms: number;
  bar_position: number;
}

export const BEAT_NUDGE_MS = 10;
export const DEFAULT_SAMPLE_RATE = 44100;

/**
 * One nudge: one CSS pixel of the zoom view, between one sample and BEAT_NUDGE_MS; `fine` (⌥) is always one sample.
 * Beat times in the app are exact (fractions of a millisecond); only storage rounds them (see `gridForStorage`).
 */
export function gridNudgeMs(msPerPixel: number, fine: boolean, msPerSample = 1000 / DEFAULT_SAMPLE_RATE): number {
  if (fine) return msPerSample;
  if (!(msPerPixel > 0)) return BEAT_NUDGE_MS;
  return Math.min(BEAT_NUDGE_MS, Math.max(msPerSample, msPerPixel));
}

/** The grid moved by `deltaMs` exactly; beats that would start before the track are dropped (a beat time is never negative). */
export function shiftBeats(beats: BeatMarker[], deltaMs: number): BeatMarker[] {
  const out: BeatMarker[] = [];
  for (const beat of beats) {
    const time = beat.time_ms + deltaMs;
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
  return positionMs - nearest;
}

/**
 * The library and the export's PQTZ grid store whole milliseconds. A grid moved by hand keeps its exact place as whole
 * ms beats plus one shared sub-millisecond offset (`grid_offset_ms` of the track's edits): every beat moves by the same
 * amount, so they all share the fraction of the first one. Each stored beat is its exact time rounded to the ms.
 */
export function gridForStorage(beats: BeatMarker[]): { beats: BeatMarker[]; offsetMs: number } {
  if (!beats.length) return { beats: [], offsetMs: 0 };
  const offsetMs = beats[0].time_ms - Math.round(beats[0].time_ms);
  return {
    beats: beats.map((b) => ({ time_ms: Math.max(0, Math.round(b.time_ms - offsetMs)), bar_position: b.bar_position })),
    offsetMs,
  };
}

/** The exact beats from stored whole-ms beats and the grid's offset. */
export function gridFromStorage(beats: BeatMarker[], offsetMs: number | null | undefined): BeatMarker[] {
  const offset = Number.isFinite(offsetMs) ? (offsetMs as number) : 0;
  return beats.map((b) => ({ time_ms: b.time_ms + offset, bar_position: b.bar_position }));
}

export { SAVE_DELAY_MS as GRID_SAVE_DELAY_MS, createPersister as createGridPersister } from './persister.ts';
export type { Persister as GridPersister } from './persister.ts';
