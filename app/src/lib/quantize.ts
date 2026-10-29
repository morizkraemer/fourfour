import type { BeatMarker } from './beatGrid.ts';

export function nearestBeatMs(beats: BeatMarker[], ms: number): number {
  if (!beats.length) return ms;
  let low = 0;
  let high = beats.length - 1;
  while (low < high) {
    const mid = (low + high) >> 1;
    if (beats[mid].time_ms < ms) low = mid + 1;
    else high = mid;
  }
  const after = beats[low].time_ms;
  const before = low > 0 ? beats[low - 1].time_ms : after;
  return Math.abs(ms - before) <= Math.abs(after - ms) ? before : after;
}

export function quantizedMs(beats: BeatMarker[], ms: number, on: boolean): number {
  return on && beats.length ? Math.max(0, Math.round(nearestBeatMs(beats, ms))) : ms;
}
