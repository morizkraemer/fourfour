const REMEMBERED_TRACKS = 500;

/**
 * Warms the high-resolution waveform of tracks the user is about to open, one at a time. A request replaces the tracks
 * still waiting (the newest selection wins) after `delayMs`, so clicking through rows does not queue a decode per row;
 * the one already measuring finishes. A track warmed once is not asked for again. `now` puts one track first without the
 * delay, for a row the pointer just went down on (a double-click loads it before a delayed request would start).
 */
export function createDetailPrefetcher(warm: (trackId: number) => Promise<void>, delayMs = 150) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let waiting: number[] = [];
  let running: number | null = null;
  const warmed = new Set<number>();

  async function next() {
    if (running !== null) return;
    const trackId = waiting.shift();
    if (trackId === undefined) return;
    running = trackId;
    const warmedNow = await warm(trackId).then(() => true, () => false);
    if (warmedNow) {
      warmed.add(trackId);
      if (warmed.size > REMEMBERED_TRACKS) warmed.delete(warmed.values().next().value!);
    }
    running = null;
    void next();
  }

  return {
    now(trackId: number) {
      if (warmed.has(trackId) || running === trackId) return;
      waiting = [trackId, ...waiting.filter((id) => id !== trackId)];
      void next();
    },
    request(trackIds: number[]) {
      clearTimeout(timer);
      timer = setTimeout(() => {
        waiting = [...new Set(trackIds)].filter((id) => !warmed.has(id) && id !== running);
        void next();
      }, delayMs);
    },
  };
}
