import { beatIntervalMs, type BeatMarker } from './beatGrid.ts';

export const BAR_BEATS = 4;
export const BIG_STEP_BARS = 4;
export const FALLBACK_STEP_MS = 1000;
export const FALLBACK_BIG_STEP_MS = 10000;
const ON_BEAT_TOLERANCE_MS = 1;

export type SeekKey = 'ArrowLeft' | 'ArrowRight' | 'Home' | 'End';

export interface SeekContext {
  positionMs: number;
  durationMs: number;
  beats: BeatMarker[];
}

export function isSeekKey(key: string): key is SeekKey {
  return key === 'ArrowLeft' || key === 'ArrowRight' || key === 'Home' || key === 'End';
}

function beatTimeAt(beats: BeatMarker[], interval: number, index: number): number {
  if (index < 0) return beats[0].time_ms + index * interval;
  if (index >= beats.length) return beats[beats.length - 1].time_ms + (index - beats.length + 1) * interval;
  return beats[index].time_ms;
}

function lastBeatAtOrBefore(beats: BeatMarker[], interval: number, positionMs: number): number {
  const limit = positionMs + ON_BEAT_TOLERANCE_MS;
  if (limit < beats[0].time_ms) return Math.floor((limit - beats[0].time_ms) / interval);
  const last = beats.length - 1;
  if (limit >= beats[last].time_ms) return last + Math.floor((limit - beats[last].time_ms) / interval);
  let index = 0;
  while (index < last && beats[index + 1].time_ms <= limit) index++;
  return index;
}

function gridTarget(beats: BeatMarker[], positionMs: number, beatCount: number): number {
  const interval = beatIntervalMs(beats) as number;
  const floor = lastBeatAtOrBefore(beats, interval, positionMs);
  if (beatCount > 0) return beatTimeAt(beats, interval, floor + beatCount);
  const onBeat = positionMs - beatTimeAt(beats, interval, floor) <= ON_BEAT_TOLERANCE_MS;
  return beatTimeAt(beats, interval, onBeat ? floor + beatCount : floor + beatCount + 1);
}

/** Where a seek key moves the playhead: a beat (or 1 s without a grid), Shift four bars (or 10 s), Home and End the track ends. */
export function seekTargetMs(key: SeekKey, shift: boolean, ctx: SeekContext): number {
  const { positionMs, durationMs, beats } = ctx;
  if (key === 'Home') return 0;
  if (key === 'End') return durationMs;
  const direction = key === 'ArrowRight' ? 1 : -1;
  const hasGrid = beats.length >= 2 && (beatIntervalMs(beats) ?? 0) > 0;
  const raw = hasGrid
    ? gridTarget(beats, positionMs, direction * (shift ? BIG_STEP_BARS * BAR_BEATS : 1))
    : positionMs + direction * (shift ? FALLBACK_BIG_STEP_MS : FALLBACK_STEP_MS);
  return Math.min(durationMs, Math.max(0, raw));
}

export function formatClock(ms: number): string {
  const seconds = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
}
