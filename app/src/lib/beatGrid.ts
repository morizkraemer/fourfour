export interface BeatMarker {
  time_ms: number;
  bar_position: number;
}

export interface EditedGrid {
  beats: BeatMarker[];
  anchorMs: number | null;
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

export function beatIntervalMs(beats: BeatMarker[]): number | null {
  if (beats.length < 2) return null;
  return (beats[beats.length - 1].time_ms - beats[0].time_ms) / (beats.length - 1);
}

/** The grid moved one beat later (`direction` 1) or earlier (-1); a beat that opens up at the track start is filled in. */
export function shiftBeatsByBeat(beats: BeatMarker[], direction: 1 | -1): BeatMarker[] {
  const interval = beatIntervalMs(beats);
  if (interval === null || !(interval > 0)) return beats;
  const shifted = shiftBeats(beats, direction * interval);
  if (direction === 1 && shifted[0] && shifted[0].time_ms - interval >= 0) {
    const first = shifted[0];
    shifted.unshift({ time_ms: first.time_ms - interval, bar_position: first.bar_position === 1 ? 4 : first.bar_position - 1 });
  }
  return shifted;
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

interface RescaleAnchor {
  storedMs: number;
  barPosition: number;
  exactMs: number;
  offsetMs: number;
}

function rescaleAnchor(beats: BeatMarker[], anchorMs: number | null | undefined): RescaleAnchor | null {
  const { beats: stored, offsetMs } = gridForStorage(beats);
  if (Number.isFinite(anchorMs)) {
    return { storedMs: Math.round((anchorMs as number) - offsetMs), barPosition: 1, exactMs: anchorMs as number, offsetMs };
  }
  const beat = stored.find((b) => b.bar_position === 1) ?? stored[0];
  if (!beat) return null;
  return { storedMs: beat.time_ms, barPosition: beat.bar_position, exactMs: beat.time_ms + offsetMs, offsetMs };
}

/** The downbeat a BPM change keeps in place: the saved anchor, else the first downbeat; null when the grid has no beats or no downbeat to keep. */
export function gridAnchorMs(beats: BeatMarker[], anchorMs: number | null | undefined): number | null {
  const anchor = rescaleAnchor(beats, anchorMs);
  return anchor && anchor.barPosition === 1 ? anchor.exactMs : null;
}

/** The anchor after the grid moved by `deltaMs`: the saved one moves with it, else the first downbeat does. */
export function shiftedAnchorMs(beats: BeatMarker[], anchorMs: number | null | undefined, deltaMs: number): number | null {
  const anchor = gridAnchorMs(beats, anchorMs);
  return anchor === null ? null : anchor + deltaMs;
}

/**
 * The grid re-spaced to `bpm` from its anchor downbeat (`anchorMs`, saved as `grid_anchor_ms`; without one, the first
 * downbeat), exactly as the library's `rescale_beat_grid` does it on the stored grid (whole-ms anchor, beats from the
 * track start to its end), plus the grid's sub-millisecond offset.
 */
export function rescaleBeats(beats: BeatMarker[], bpm: number, durationMs: number, anchorMs?: number | null): BeatMarker[] {
  const anchor = rescaleAnchor(beats, anchorMs);
  if (!anchor) return [];
  const tempo = Math.round(bpm * 100);
  const beatMs = 6_000_000 / tempo;
  const firstIndex = -Math.floor(anchor.storedMs / beatMs);
  const lastIndex = Math.max(0, Math.floor((durationMs - anchor.storedMs) / beatMs));
  const out: BeatMarker[] = [];
  for (let k = firstIndex; k <= lastIndex; k++) {
    out.push({
      time_ms: Math.round(anchor.storedMs + k * beatMs) + anchor.offsetMs,
      bar_position: ((((anchor.barPosition - 1 + k) % 4) + 4) % 4) + 1,
    });
  }
  return out;
}

export { SAVE_DELAY_MS as GRID_SAVE_DELAY_MS, createPersister as createGridPersister } from './persister.ts';
export type { Persister as GridPersister } from './persister.ts';
