const REMEMBERED_TRACKS = 500;

/**
 * Warms the high-resolution waveform of tracks the user is about to open, one at a time. A request replaces the tracks
 * still waiting (the newest selection wins) after `delayMs`, so clicking through rows does not queue a decode per row;
 * the one already measuring finishes. A track warmed once is not asked for again. `now` warms one track at once and
 * `urgent`, beside the queue, for a row the pointer just went down on: its click opens it a moment later and must not
 * wait for a background warm.
 */
export function createDetailPrefetcher(warm: (trackId: number, urgent: boolean) => Promise<void>, delayMs = 150) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let waiting: number[] = [];
  let running: number | null = null;
  const urgent = new Set<number>();
  const warmed = new Set<number>();

  async function next() {
    if (running !== null) return;
    const trackId = waiting.shift();
    if (trackId === undefined) return;
    running = trackId;
    if (await warm(trackId, false).then(() => true, () => false)) remember(trackId);
    running = null;
    void next();
  }

  function remember(trackId: number) {
    warmed.add(trackId);
    if (warmed.size > REMEMBERED_TRACKS) warmed.delete(warmed.values().next().value!);
  }

  return {
    now(trackId: number) {
      if (warmed.has(trackId) || urgent.has(trackId)) return;
      waiting = waiting.filter((id) => id !== trackId);
      urgent.add(trackId);
      void warm(trackId, true).then(
        () => remember(trackId),
        () => {},
      ).finally(() => urgent.delete(trackId));
    },
    request(trackIds: number[]) {
      clearTimeout(timer);
      timer = setTimeout(() => {
        waiting = [...new Set(trackIds)].filter((id) => !warmed.has(id) && id !== running && !urgent.has(id));
        void next();
      }, delayMs);
    },
  };
}
