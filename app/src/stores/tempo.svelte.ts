/**
 * tempo.svelte.ts — the tempo and key the player is playing at, as opposed to the analysed BPM and
 * key stored with the track. The engine does the work; this holds what the controls show and
 * pushes changes to it. Tempo and key shift go back to the track's own values on every load,
 * range and key lock stay as set.
 */

import {
  playbackSetKeyLock,
  playbackSetKeyShift,
  playbackSetTempo,
} from '../services/playback.svelte.ts';
import {
  DEFAULT_TEMPO_RANGE,
  clampPercent,
  clampShift,
  ratioFromPercent,
  smallestRangeFor,
  type TempoRange,
} from '../lib/tempo.ts';

export const tempo = $state({
  percent: 0,
  keyShift: 0,
  keyLock: true,
  range: DEFAULT_TEMPO_RANGE as TempoRange,
  liveRatio: 1,
});

function pushed(what: string, request: Promise<void>) {
  request.catch((err) => console.error(`${what} failed:`, err));
}

export function tempoActive(): boolean {
  return tempo.percent !== 0 || tempo.keyShift !== 0;
}

/** Typed values may go past the selected range; the range widens to hold them. */
export function setTempoPercent(percent: number) {
  const next = clampPercent(percent);
  tempo.percent = next;
  if (Math.abs(next) > tempo.range) tempo.range = smallestRangeFor(next);
  pushed('Setting the tempo', playbackSetTempo(ratioFromPercent(next)));
}

export function setTempoRange(range: TempoRange) {
  tempo.range = range;
  if (Math.abs(tempo.percent) > range) setTempoPercent(clampPercent(tempo.percent, range));
}

export function setKeyShift(semitones: number) {
  tempo.keyShift = clampShift(semitones);
  pushed('Setting the key shift', playbackSetKeyShift(tempo.keyShift));
}

export function setKeyLock(on: boolean) {
  tempo.keyLock = on;
  pushed('Setting key lock', playbackSetKeyLock(on));
}

export function resetTempo() {
  tempo.percent = 0;
  tempo.keyShift = 0;
  pushed('Resetting the tempo', playbackSetTempo(1));
  pushed('Resetting the key shift', playbackSetKeyShift(0));
}

/** The engine already drops tempo and key shift when a track is unloaded; this is the page's copy. */
export function resetTempoForNewTrack() {
  tempo.percent = 0;
  tempo.keyShift = 0;
  tempo.liveRatio = 1;
}
