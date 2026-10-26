export const BPM_MIN = 40;
export const BPM_MAX = 250;

const BPM_STEP = { fine: 0.01, normal: 0.1, coarse: 1 };

export function tierOf(event) {
  if (event.shiftKey) return 'coarse';
  if (event.altKey) return 'fine';
  return 'normal';
}

function round2(value) {
  return Math.round(value * 100) / 100;
}

export function clampBpm(value) {
  return Math.min(BPM_MAX, Math.max(BPM_MIN, value));
}

export function stepBpm(value, units, tier = 'normal') {
  const step = BPM_STEP[tier];
  const position = value / step;
  const snapped = units > 0 ? Math.floor(position + 1e-9) : Math.ceil(position - 1e-9);
  return clampBpm(round2((snapped + units) * step));
}

export function doubleBpm(value) {
  const next = round2(value * 2);
  return next > BPM_MAX ? null : next;
}

export function halveBpm(value) {
  const next = round2(value / 2);
  return next < BPM_MIN ? null : next;
}

export function parseBpm(text) {
  const cleaned = String(text ?? '').trim().replace(',', '.');
  if (!/^\d+(\.\d+)?$/.test(cleaned)) return null;
  const value = round2(Number(cleaned));
  return value >= BPM_MIN && value <= BPM_MAX ? value : null;
}

export function formatBpm(value) {
  return Number.isFinite(value) && value > 0 ? value.toFixed(2) : '—';
}

const WHEEL_SIZE = 12;
const SEMITONE_ON_WHEEL = 7;

const MINOR_PITCH_CLASSES = [8, 3, 10, 5, 0, 7, 2, 9, 4, 11, 6, 1];
const MAJOR_PITCH_CLASSES = [11, 6, 1, 8, 3, 10, 5, 0, 7, 2, 9, 4];
const NATURAL_PITCH_CLASS = { C: 0, D: 2, E: 4, F: 5, G: 7, A: 9, B: 11 };

function parseNoteName(raw) {
  const match = /^([A-Ga-g])([#♯b♭]?)\s*(m|min|minor|M|maj|major)?$/.exec(raw);
  if (!match) return null;
  const accidental = match[2] === '#' || match[2] === '♯' ? 1 : match[2] ? -1 : 0;
  const pitchClass = (NATURAL_PITCH_CLASS[match[1].toUpperCase()] + accidental + 12) % 12;
  const minor = /^(m|min|minor)$/.test(match[3] ?? '');
  const index = (minor ? MINOR_PITCH_CLASSES : MAJOR_PITCH_CLASSES).indexOf(pitchClass);
  return { number: index + 1, letter: minor ? 'A' : 'B' };
}

export function parseCamelot(text) {
  const raw = String(text ?? '').trim();
  const wheel = /^(\d{1,2})\s*([abAB])$/.exec(raw);
  if (wheel) {
    const number = Number(wheel[1]);
    return number >= 1 && number <= WHEEL_SIZE ? { number, letter: wheel[2].toUpperCase() } : null;
  }
  return parseNoteName(raw);
}

export function parseKey(text) {
  const parsed = parseCamelot(text);
  return parsed ? `${parsed.number}${parsed.letter}` : null;
}

export function stepKey(key, units, mod = 'normal') {
  const parsed = parseCamelot(key) ?? { number: 1, letter: 'A' };
  if (mod === 'toggle') {
    const flips = Math.abs(units) % 2 === 1;
    const letter = flips ? (parsed.letter === 'A' ? 'B' : 'A') : parsed.letter;
    return `${parsed.number}${letter}`;
  }
  const move = mod === 'coarse' ? units * SEMITONE_ON_WHEEL : units;
  const number = ((((parsed.number - 1 + move) % WHEEL_SIZE) + WHEEL_SIZE) % WHEEL_SIZE) + 1;
  return `${number}${parsed.letter}`;
}

export function keyPosition(key) {
  const parsed = parseCamelot(key);
  return parsed ? (parsed.number - 1) * 2 + (parsed.letter === 'A' ? 1 : 2) : 0;
}

export function keyModifier(event) {
  if (event.altKey) return 'toggle';
  if (event.shiftKey) return 'coarse';
  return 'normal';
}

export const bpmScrub = {
  kind: 'bpm',
  min: BPM_MIN,
  max: BPM_MAX,
  pxPerUnit: 6,
  tier: tierOf,
  step: stepBpm,
  parse: parseBpm,
  format: formatBpm,
  valid: (value) => Number.isFinite(value) && value > 0,
  editText: (value) => value.toFixed(2),
  ariaNow: (value) => value,
  ariaText: (value) => `${formatBpm(value)} BPM`,
  hint: 'Drag, scroll or arrow keys. Alt fine, Shift coarse. Double-click to type.',
  actions: [
    { label: 'Double (×2)', shortcut: '⌘↑', hotkey: 'ArrowUp', apply: doubleBpm },
    { label: 'Halve (÷2)', shortcut: '⌘↓', hotkey: 'ArrowDown', apply: halveBpm },
  ],
};

export const keyScrub = {
  kind: 'key',
  min: 1,
  max: 24,
  pxPerUnit: 14,
  tier: keyModifier,
  step: stepKey,
  parse: parseKey,
  format: (value) => value || '—',
  valid: (value) => parseCamelot(value) !== null,
  editText: (value) => value,
  ariaNow: keyPosition,
  ariaText: (value) => `Camelot ${value}`,
  hint: 'Drag, scroll or arrow keys moves the Camelot wheel. Alt swaps A/B, Shift jumps a semitone. Double-click to type.',
  actions: [
    { label: 'Relative major / minor', apply: (value) => stepKey(value, 1, 'toggle') },
    { label: 'Up a semitone (+7)', apply: (value) => stepKey(value, 1, 'coarse') },
    { label: 'Down a semitone (−7)', apply: (value) => stepKey(value, -1, 'coarse') },
  ],
};
