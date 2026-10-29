export const TAP_RESET_MS = 2000;
export const TAPS_NEEDED = 4;

export interface TapTempo {
  tap(): number | null;
  reset(): void;
}

/** Tap along to the beat: from the fourth tap on, the BPM of the average interval; a pause of 2 s starts over. */
export function createTapTempo(now: () => number = () => performance.now()): TapTempo {
  let taps: number[] = [];
  return {
    tap() {
      const at = now();
      if (taps.length > 0 && at - taps[taps.length - 1] > TAP_RESET_MS) taps = [];
      taps.push(at);
      if (taps.length < TAPS_NEEDED) return null;
      const averageMs = (taps[taps.length - 1] - taps[0]) / (taps.length - 1);
      return Math.round((60000 / averageMs) * 100) / 100;
    },
    reset() {
      taps = [];
    },
  };
}
