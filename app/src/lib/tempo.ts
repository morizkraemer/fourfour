import {
  clampBpm,
  formatBpm,
  parseBpm,
  parseKey,
  stepBpm,
  tierOf,
} from '../../../prototyping/viewport/design-system/utils/scrub.js';

export const TEMPO_RANGES = [8, 16, 50] as const;
export type TempoRange = (typeof TEMPO_RANGES)[number];
export const DEFAULT_TEMPO_RANGE: TempoRange = 8;
export const MAX_SEMITONES = 12;

type Tier = 'fine' | 'normal' | 'coarse';

const PERCENT_STEPS: Record<TempoRange, Record<Tier, number>> = {
  8: { fine: 0.01, normal: 0.02, coarse: 0.1 },
  16: { fine: 0.02, normal: 0.05, coarse: 0.25 },
  50: { fine: 0.05, normal: 0.1, coarse: 0.5 },
};

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

export function clampPercent(percent: number, range: number = 50): number {
  if (!Number.isFinite(percent)) return 0;
  return Math.min(range, Math.max(-range, percent));
}

export function smallestRangeFor(percent: number): TempoRange {
  const magnitude = Math.abs(percent);
  return TEMPO_RANGES.find((range) => magnitude <= range) ?? 50;
}

export function stepPercent(percent: number, units: number, tier: Tier, range: TempoRange): number {
  const step = PERCENT_STEPS[range][tier];
  const position = percent / step;
  const snapped = units > 0 ? Math.floor(position + 1e-9) : Math.ceil(position - 1e-9);
  return clampPercent(round((snapped + units) * step, 4), range);
}

export function playingBpm(analysedBpm: number, percent: number): number {
  return analysedBpm * ratioFromPercent(percent);
}

export function percentForBpm(targetBpm: number, analysedBpm: number): number {
  return clampPercent(round((targetBpm / analysedBpm - 1) * 100, 4));
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
  return Math.abs(value) <= 50 ? value : null;
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

/** BPM field of the player: shows the BPM being played, steps and typing set the tempo that reaches it. */
export function playingBpmScrub(analysedBpm: number, range: TempoRange) {
  return {
    kind: 'bpm',
    min: analysedBpm * ratioFromPercent(-50),
    max: analysedBpm * ratioFromPercent(50),
    pxPerUnit: 6,
    tier: tierOf,
    step: (value: number, units: number, tier: Tier = 'normal') => {
      const next = stepBpm(value, units, tier);
      const lowest = analysedBpm * ratioFromPercent(-range);
      const highest = analysedBpm * ratioFromPercent(range);
      return clampBpm(round(Math.min(highest, Math.max(lowest, next)), 2));
    },
    parse: (text: string) => {
      const bpm = parseBpm(text);
      if (bpm === null) return null;
      const percent = (bpm / analysedBpm - 1) * 100;
      return Math.abs(percent) <= 50 + 1e-9 ? bpm : null;
    },
    format: formatBpm,
    valid: (value: number) => Number.isFinite(value) && value > 0,
    editText: (value: number) => value.toFixed(2),
    ariaNow: (value: number) => value,
    ariaText: (value: number) => `${formatBpm(value)} BPM, ${formatPercent(percentForBpm(value, analysedBpm))}`,
    hint: 'The BPM being played. Drag, scroll or arrow keys change the tempo. Alt fine, Shift coarse. Double-click to type a BPM.',
    actions: [],
  };
}

/** Tempo as a percentage of the track's own speed, with CDJ-like fine steps per range. */
export function tempoPercentScrub(range: TempoRange) {
  return {
    kind: 'tempo',
    min: -range,
    max: range,
    pxPerUnit: 4,
    tier: tierOf,
    step: (value: number, units: number, tier: Tier = 'normal') => stepPercent(value, units, tier, range),
    parse: parsePercent,
    format: formatPercent,
    valid: (value: number) => Number.isFinite(value),
    editText: (value: number) => (value > 0 ? '+' : '') + value.toFixed(2),
    ariaNow: (value: number) => value,
    ariaText: (value: number) => `Tempo ${formatPercent(value)}`,
    hint: `Tempo offset, ±${range} %. Drag, scroll or arrow keys. Alt fine, Shift coarse. Double-click to type. ⌘0 resets.`,
    actions: [{ label: 'Reset tempo to 0 %', shortcut: '⌘0', hotkey: '0', apply: () => 0 }],
  };
}

/** KEY field of the player: the value is the shift in semitones, shown as the resulting key. */
export function keyShiftScrub(analysedKey: string, currentShift: number) {
  const resulting = (shift: number) => shiftedKey(analysedKey, shift) ?? '—';
  return {
    kind: 'key',
    min: -MAX_SEMITONES,
    max: MAX_SEMITONES,
    pxPerUnit: 14,
    tier: () => 'normal',
    step: (value: number, units: number) => clampShift(value + units),
    parse: (text: string) => shiftForKey(analysedKey, text, currentShift),
    format: resulting,
    valid: (value: number) => Number.isFinite(value) && shiftedKey(analysedKey, 0) !== null,
    editText: resulting,
    ariaNow: (value: number) => value,
    ariaText: (value: number) => `${resulting(value)}, ${formatShift(value)}`,
    hint: 'The key being played. Drag, scroll or arrow keys shift it by a semitone. Double-click to type a key.',
    actions: [{ label: 'Reset key shift', apply: () => 0 }],
  };
}
