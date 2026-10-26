export interface BeatMarker {
  time_ms: number;
  bar_position: number;
}

export const BEAT_NUDGE_MS = 10;
export const GRID_SAVE_DELAY_MS = 400;

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

export interface GridPersister {
  schedule(trackId: number, beats: BeatMarker[]): void;
  flush(): Promise<void>;
}

interface Timers {
  set(fn: () => void, ms: number): unknown;
  clear(handle: unknown): void;
}

const realTimers: Timers = {
  set: (fn, ms) => setTimeout(fn, ms),
  clear: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
};

/**
 * Saves the latest edited grid once the edits pause. Only the newest grid of a burst of nudges is written, writes
 * never overlap, and `flush` writes what is waiting right away (before another track loads).
 */
export function createGridPersister(
  save: (trackId: number, beats: BeatMarker[]) => Promise<void>,
  onError: (error: unknown) => void,
  delayMs = GRID_SAVE_DELAY_MS,
  timers: Timers = realTimers,
): GridPersister {
  let waiting: { trackId: number; beats: BeatMarker[] } | null = null;
  let timer: unknown = null;
  let writing: Promise<void> = Promise.resolve();

  const write = () => {
    timer = null;
    const job = waiting;
    waiting = null;
    if (!job) return writing;
    writing = writing.then(() => save(job.trackId, job.beats)).catch(onError);
    return writing;
  };

  return {
    schedule(trackId, beats) {
      waiting = { trackId, beats };
      if (timer !== null) timers.clear(timer);
      timer = timers.set(write, delayMs);
    },
    flush() {
      if (timer !== null) timers.clear(timer);
      return write();
    },
  };
}
