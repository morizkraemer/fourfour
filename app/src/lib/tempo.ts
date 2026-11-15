import { BPM_MAX, BPM_MIN, doubleBpm, formatBpm, halveBpm, parseKey, stepBpm, tierOf } from '../../../prototyping/viewport/design-system/utils/scrub.js';

export { BPM_MAX, BPM_MIN, doubleBpm, formatBpm, halveBpm, stepBpm, tierOf };

export const MIN_RATIO = 0.25;
export const MAX_RATIO = 4;
export const MAX_SEMITONES = 12;
export const MAX_BPM_ENTRY = 999;

export type Tier = 'fine' | 'normal' | 'coarse';

const PERCENT_STEPS: Record<Tier, number> = { fine: 0.01, normal: 0.1, coarse: 1 };

function round(value: number, places: number): number {
  const factor = 10 ** places;
  return Math.round(value * factor) / factor;
}

export function ratioFromPercent(percent: number): number {
  return 1 + percent / 100;
}

export function percentFromRatio(ratio: number): number {
  return round((ratio - 1) * 100, 6);
}

export function clampPercent(percent: number): number {
  if (!Number.isFinite(percent)) return 0;
  return Math.min(percentFromRatio(MAX_RATIO), Math.max(percentFromRatio(MIN_RATIO), percent));
}

export function stepPercent(percent: number, units: number, tier: Tier): number {
  const step = PERCENT_STEPS[tier];
  const position = percent / step;
  const snapped = units > 0 ? Math.floor(position + 1e-9) : Math.ceil(position - 1e-9);
  return clampPercent(round((snapped + units) * step, 4));
}

export function playingBpm(analysedBpm: number, percent: number): number {
  return analysedBpm * ratioFromPercent(percent);
}

export function bpmDiffers(analysedBpm: number, playedBpm: number): boolean {
  return Number.isFinite(analysedBpm) && Number.isFinite(playedBpm) && formatBpm(analysedBpm) !== formatBpm(playedBpm);
}

export function percentForBpm(targetBpm: number, analysedBpm: number): number {
  return clampPercent(round((targetBpm / analysedBpm - 1) * 100, 4));
}

/** A typed BPM: any number up to 999; the engine limits how far the tempo can actually go. */
export function parseBpmEntry(text: string): number | null {
  const cleaned = String(text ?? '').trim().replace(',', '.');
  if (!/^\d+(\.\d+)?$/.test(cleaned)) return null;
  const value = round(Number(cleaned), 2);
  return value <= MAX_BPM_ENTRY ? value : null;
}

export function formatPercent(percent: number): string {
  if (!Number.isFinite(percent)) return '—';
  const magnitude = Math.abs(percent).toFixed(2);
  if (Number(magnitude) === 0) return '0.00%';
  return `${percent < 0 ? '−' : '+'}${magnitude}%`;
}

export function parsePercent(text: string): number | null {
  const cleaned = String(text ?? '')
    .trim()
    .replace(',', '.')
    .replace('−', '-')
    .replace('%', '')
    .trim();
  if (!/^[+-]?\d+(\.\d+)?$/.test(cleaned)) return null;
  const value = round(Number(cleaned), 2);
  return value >= percentFromRatio(MIN_RATIO) && value <= percentFromRatio(MAX_RATIO) ? value : null;
}

export function clampShift(semitones: number): number {
  if (!Number.isFinite(semitones)) return 0;
  return Math.min(MAX_SEMITONES, Math.max(-MAX_SEMITONES, Math.round(semitones)));
}

export function formatShift(semitones: number): string {
  if (semitones === 0) return '0 st';
  return `${semitones < 0 ? '−' : '+'}${Math.abs(semitones)} st`;
}

const MINOR_PITCH_CLASSES = [8, 3, 10, 5, 0, 7, 2, 9, 4, 11, 6, 1];
const MAJOR_PITCH_CLASSES = [11, 6, 1, 8, 3, 10, 5, 0, 7, 2, 9, 4];
const SHARP_NAMES = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B'];
const FLAT_NAMES = ['C', 'Db', 'D', 'Eb', 'E', 'F', 'Gb', 'G', 'Ab', 'A', 'Bb', 'B'];

interface KeyParts {
  pitchClass: number;
  minor: boolean;
}

function keyParts(key: string): KeyParts | null {
  const camelot = parseKey(key);
  if (!camelot) return null;
  const number = Number(camelot.slice(0, -1));
  const minor = camelot.endsWith('A');
  const table = minor ? MINOR_PITCH_CLASSES : MAJOR_PITCH_CLASSES;
  return { pitchClass: table[number - 1], minor };
}

function camelotOf(parts: KeyParts): string {
  const table = parts.minor ? MINOR_PITCH_CLASSES : MAJOR_PITCH_CLASSES;
  return `${table.indexOf(parts.pitchClass) + 1}${parts.minor ? 'A' : 'B'}`;
}

function isCamelotNotation(key: string): boolean {
  return /^\s*\d{1,2}\s*[abAB]\s*$/.test(key);
}

/** The key after shifting `semitones`, written in the notation the analysed key is written in. */
export function shiftedKey(analysedKey: string, semitones: number): string | null {
  const parts = keyParts(analysedKey);
  if (!parts) return null;
  const pitchClass = (((parts.pitchClass + Math.round(semitones)) % 12) + 12) % 12;
  const shifted = { pitchClass, minor: parts.minor };
  if (isCamelotNotation(analysedKey)) return camelotOf(shifted);
  const names = /[b♭]/.test(analysedKey.trim().slice(1, 2)) ? FLAT_NAMES : SHARP_NAMES;
  return `${names[pitchClass]}${parts.minor ? 'm' : ''}`;
}

/** The key `units` steps around the Camelot wheel, written in the notation `key` is written in. */
export function camelotStep(key: string, units: number): string | null {
  const camelot = parseKey(key);
  if (!camelot) return null;
  const number = ((((Number(camelot.slice(0, -1)) - 1 + Math.round(units)) % 12) + 12) % 12) + 1;
  const stepped = `${number}${camelot.slice(-1)}`;
  if (isCamelotNotation(key)) return stepped;
  const parts = keyParts(stepped);
  if (!parts) return null;
  const names = /[b♭]/.test(key.trim().slice(1, 2)) ? FLAT_NAMES : SHARP_NAMES;
  return `${names[parts.pitchClass]}${parts.minor ? 'm' : ''}`;
}

const CAMELOT_KEY_NAMES: Record<string, string> = {
  '1A': 'A♭m', '2A': 'E♭m', '3A': 'B♭m', '4A': 'Fm', '5A': 'Cm', '6A': 'Gm',
  '7A': 'Dm', '8A': 'Am', '9A': 'Em', '10A': 'Bm', '11A': 'F♯m', '12A': 'D♭m',
  '1B': 'B', '2B': 'F♯', '3B': 'D♭', '4B': 'A♭', '5B': 'E♭', '6B': 'B♭',
  '7B': 'F', '8B': 'C', '9B': 'G', '10B': 'D', '11B': 'A', '12B': 'E',
};

export interface KeyCell {
  camelot: string;
  name: string;
}

export function camelotOfKey(key: string): string | null {
  return parseKey(key);
}

/** The 12 keys of one mode in Camelot order, each with its musical name. */
export function keyCells(minor: boolean): KeyCell[] {
  return Array.from({ length: 12 }, (_, i) => {
    const camelot = `${i + 1}${minor ? 'A' : 'B'}`;
    return { camelot, name: CAMELOT_KEY_NAMES[camelot] };
  });
}

/** `camelot` written the way `like` is written: as a Camelot code, or as a note name. */
export function keyWrittenLike(camelot: string, like: string): string {
  if (isCamelotNotation(like)) return camelot;
  const parts = keyParts(camelot);
  if (!parts) return camelot;
  const names = /[b♭]/.test(like.trim().slice(1, 2)) ? FLAT_NAMES : SHARP_NAMES;
  return `${names[parts.pitchClass]}${parts.minor ? 'm' : ''}`;
}

/** The semitones the pitch has moved: the key shift, plus the tempo change when key lock is off and the tempo changes the pitch. */
export function effectiveShift(keyShift: number, percent: number, keyLock: boolean): number {
  if (keyLock) return keyShift;
  return keyShift + Math.round(12 * Math.log2(ratioFromPercent(percent)));
}

/** The shift that turns `analysedKey` into `targetKey`, the one closest to `currentShift`. A different mode cannot be reached by shifting. */
export function shiftForKey(analysedKey: string, targetKey: string, currentShift = 0): number | null {
  const from = keyParts(analysedKey);
  const to = keyParts(targetKey);
  if (!from || !to || from.minor !== to.minor) return null;
  const base = (((to.pitchClass - from.pitchClass) % 12) + 12) % 12;
  const candidates = [base - 12, base, base + 12].filter((s) => Math.abs(s) <= MAX_SEMITONES);
  return candidates.reduce((best, s) => {
    const nearer = Math.abs(s - currentShift) - Math.abs(best - currentShift);
    return nearer < 0 || (nearer === 0 && Math.abs(s) < Math.abs(best)) ? s : best;
  });
}

export const BPM_CLICK_STEP = 1;
export const BPM_CLICK_STEP_FINE = 0.1;

export const DRAG_PX_PER_UNIT = { bpm: 1.5, percent: 1.5, key: 8 } as const;

const BPM_DRAG_STEP: Record<Tier, number> = { fine: 0.01, normal: 0.1, coarse: 1 };

export function bpmDragStep(tier: Tier): number {
  return BPM_DRAG_STEP[tier];
}

/** The tempo that makes the track play `units` steps of `bpmStep` away from where it plays now. */
export function percentAfterBpmStep(
  analysedBpm: number,
  percent: number,
  units: number,
  bpmStep: number,
): number {
  const next = round(playingBpm(analysedBpm, percent) + units * bpmStep, 2);
  return clampPercent(round((next / analysedBpm - 1) * 100, 4));
}

/** A typed key: Camelot or note name picks the key, a number (`+2`, `-3`, `2st`) is the shift in semitones. */
export function parseKeyEntry(text: string, analysedKey: string, currentShift = 0): number | null {
  const cleaned = String(text ?? '').trim().replace('−', '-');
  const semitones = /^([+-]?\d+)\s*(st)?$/i.exec(cleaned);
  if (semitones) {
    const shift = Number(semitones[1]);
    return Math.abs(shift) <= MAX_SEMITONES ? shift : null;
  }
  return shiftForKey(analysedKey, cleaned, currentShift);
}
