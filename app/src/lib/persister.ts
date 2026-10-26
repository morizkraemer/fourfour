export const SAVE_DELAY_MS = 400;

export interface Persister<T> {
  schedule(trackId: number, value: T): void;
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
 * Saves the latest edited value once the edits pause. Only the newest value of a burst of edits is written,
 * writes never overlap, an edit of another track writes the waiting one first, and `flush` writes what is
 * waiting right away (before another track loads).
 */
export function createPersister<T>(
  save: (trackId: number, value: T) => Promise<void>,
  onError: (error: unknown) => void,
  delayMs = SAVE_DELAY_MS,
  timers: Timers = realTimers,
): Persister<T> {
  let waiting: { trackId: number; value: T } | null = null;
  let timer: unknown = null;
  let writing: Promise<void> = Promise.resolve();

  const write = () => {
    timer = null;
    const job = waiting;
    waiting = null;
    if (!job) return writing;
    writing = writing.then(() => save(job.trackId, job.value)).catch(onError);
    return writing;
  };

  return {
    schedule(trackId, value) {
      if (waiting && waiting.trackId !== trackId) {
        if (timer !== null) timers.clear(timer);
        write();
      }
      waiting = { trackId, value };
      if (timer !== null) timers.clear(timer);
      timer = timers.set(write, delayMs);
    },
    flush() {
      if (timer !== null) timers.clear(timer);
      return write();
    },
  };
}
