import type { BeatMarker } from '../lib/beatGrid.ts';
import { DEFAULT_QUANTIZE_DIVISION, quantizedMs, type QuantizeDivision } from '../lib/quantize.ts';

export const QUANTIZE_STORAGE_KEY = 'fourfour.quantize.v1';

function loadQuantize(): boolean {
  try {
    return localStorage.getItem(QUANTIZE_STORAGE_KEY) !== '0';
  } catch {
    return true;
  }
}

export const quantize = $state<{ on: boolean; division: QuantizeDivision }>({ on: loadQuantize(), division: DEFAULT_QUANTIZE_DIVISION });

export function setQuantize(on: boolean) {
  quantize.on = on;
  try {
    localStorage.setItem(QUANTIZE_STORAGE_KEY, on ? '1' : '0');
  } catch {
    return;
  }
}

/** `ms` on the quantize grid step when quantize is on and the track has a grid, else unchanged. */
export function quantizeMs(beats: BeatMarker[], ms: number): number {
  return quantizedMs(beats, ms, quantize.on, quantize.division);
}
