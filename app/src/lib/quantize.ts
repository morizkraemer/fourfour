import type { BeatMarker } from './beatGrid.ts';

/** Note value cues snap to with quantize on: 4 = beat, 8 = eighth, 16 = sixteenth (a beat is a quarter note). */
export type QuantizeDivision = 4 | 8 | 16;

export const DEFAULT_QUANTIZE_DIVISION: QuantizeDivision = 16;

function lastBeatAtOrBefore(beats: BeatMarker[], ms: number): number {
  let low = 0;
  let high = beats.length - 1;
  while (low < high) {
    const mid = (low + high + 1) >> 1;
    if (beats[mid].time_ms <= ms) low = mid;
    else high = mid - 1;
  }
  return low;
}

/**
 * The grid step (`division` 16 = a sixteenth) nearest to `ms`. Each beat-to-beat span is split evenly, so the steps follow
 * tempo changes. Before the first beat the first span repeats backwards and after the last beat the last span repeats
 * forwards; a step never lands before 0. A grid of one beat snaps to that beat; ties go to the earlier step.
 */
export function nearestGridStepMs(beats: BeatMarker[], ms: number, division: QuantizeDivision = DEFAULT_QUANTIZE_DIVISION): number {
  if (!beats.length) return ms;
  if (beats.length === 1) return beats[0].time_ms;
  const stepsPerBeat = division / 4;
  const last = beats.length - 1;
  const i = ms < beats[0].time_ms ? 0 : Math.min(lastBeatAtOrBefore(beats, ms), last - 1);
  const start = beats[i].time_ms;
  const step = (beats[i + 1].time_ms - start) / stepsPerBeat;
  if (!(step > 0)) return start;
  const steps = (ms - start) / step;
  let snapped = start + Math.ceil(steps - 0.5) * step;
  if (snapped < 0) snapped += step * Math.ceil(-snapped / step);
  return snapped;
}

export function quantizedMs(beats: BeatMarker[], ms: number, on: boolean, division: QuantizeDivision = DEFAULT_QUANTIZE_DIVISION): number {
  return on && beats.length ? Math.max(0, Math.round(nearestGridStepMs(beats, ms, division))) : ms;
}
