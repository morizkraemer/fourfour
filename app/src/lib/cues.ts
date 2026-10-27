export const MEMORY_SLOT = 0;
export const HOT_CUE_SLOTS = 8;
export const MEMORY_MERGE_MS = 50;

export interface Cue {
  slot: number;
  timeMs: number;
  loopMs: number | null;
  color: string | null;
  name: string;
}

export interface CuePointPayload {
  hot_cue_number: number;
  time_ms: number;
  loop_time_ms?: number | null;
  color?: string | null;
  name?: string | null;
}

export interface CuePalette {
  name: string;
  color: string;
}

export const CUE_PALETTE: readonly CuePalette[] = [
  { name: 'Pink', color: '#ff2fa3' },
  { name: 'Red', color: '#ff3b30' },
  { name: 'Orange', color: '#ff8a00' },
  { name: 'Yellow', color: '#ffd600' },
  { name: 'Lime', color: '#a8e200' },
  { name: 'Green', color: '#1fd35a' },
  { name: 'Mint', color: '#17d9a3' },
  { name: 'Aqua', color: '#1fd0e8' },
  { name: 'Sky', color: '#33a1ff' },
  { name: 'Blue', color: '#3b5bff' },
  { name: 'Indigo', color: '#6a4cff' },
  { name: 'Purple', color: '#a64cff' },
  { name: 'Magenta', color: '#d83cff' },
  { name: 'Rose', color: '#ff5c8a' },
  { name: 'Peach', color: '#ffa07a' },
  { name: 'White', color: '#e6e6e6' },
];

const HOT_DEFAULT_COLORS = ['#1fd35a', '#33a1ff', '#ffd600', '#ff8a00', '#ff2fa3', '#a64cff', '#1fd0e8', '#a8e200'];
const MEMORY_COLOR = '#ff3b30';

export function cueKey(cue: Pick<Cue, 'slot' | 'timeMs'>): string {
  return cue.slot === MEMORY_SLOT ? `m${cue.timeMs}` : `h${cue.slot}`;
}

export function slotLetter(slot: number): string {
  return slot === MEMORY_SLOT ? 'M' : String.fromCharCode(64 + slot);
}

export function cueColor(cue: Pick<Cue, 'slot' | 'color'>): string {
  const color = cue.color;
  if (color && /^#[0-9a-f]{6}$/i.test(color)) return color;
  const index = color ? /^table:(\d+)$/.exec(color) : null;
  if (index) return CUE_PALETTE[(Number(index[1]) - 1) % CUE_PALETTE.length].color;
  return cue.slot === MEMORY_SLOT ? MEMORY_COLOR : HOT_DEFAULT_COLORS[(cue.slot - 1) % HOT_DEFAULT_COLORS.length];
}

export function formatCueTime(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, '0')}`;
}

export function sortCues(cues: Cue[]): Cue[] {
  return [...cues].sort((a, b) => {
    if ((a.slot === MEMORY_SLOT) !== (b.slot === MEMORY_SLOT)) return a.slot === MEMORY_SLOT ? 1 : -1;
    return a.slot - b.slot || a.timeMs - b.timeMs;
  });
}

export function cuesFromPayload(points: CuePointPayload[] | null | undefined): Cue[] {
  return sortCues(
    (points ?? []).map((p) => ({
      slot: p.hot_cue_number ?? MEMORY_SLOT,
      timeMs: p.time_ms ?? 0,
      loopMs: p.loop_time_ms ?? null,
      color: p.color ?? null,
      name: p.name ?? '',
    })),
  );
}

export function cuesToPayload(cues: Cue[]): CuePointPayload[] {
  return cues.map((c) => ({
    hot_cue_number: c.slot,
    time_ms: c.timeMs,
    loop_time_ms: c.loopMs,
    color: c.color,
    name: c.name,
  }));
}

export function hotCues(cues: Cue[]): Cue[] {
  return cues.filter((c) => c.slot !== MEMORY_SLOT);
}

export function memoryCues(cues: Cue[]): Cue[] {
  return cues.filter((c) => c.slot === MEMORY_SLOT);
}

export function hotCueIn(cues: Cue[], slot: number): Cue | undefined {
  return cues.find((c) => c.slot === slot);
}

export function firstFreeSlot(cues: Cue[]): number | null {
  for (let slot = 1; slot <= HOT_CUE_SLOTS; slot++) if (!hotCueIn(cues, slot)) return slot;
  return null;
}

/** Puts a hot cue at `timeMs` in `slot` (the first free one when none is given); null when every slot is taken or the slot is. */
export function withHotCue(cues: Cue[], timeMs: number, slot: number | null = null): { cues: Cue[]; cue: Cue } | null {
  const target = slot ?? firstFreeSlot(cues);
  if (target === null || target < 1 || target > HOT_CUE_SLOTS || hotCueIn(cues, target)) return null;
  const cue: Cue = { slot: target, timeMs: Math.round(timeMs), loopMs: null, color: null, name: '' };
  return { cues: sortCues([...cues, cue]), cue };
}

/** Adds a memory cue; one within `MEMORY_MERGE_MS` of the position is the same cue and nothing changes. */
export function withMemoryCue(cues: Cue[], timeMs: number): { cues: Cue[]; cue: Cue; added: boolean } {
  const near = memoryCues(cues).find((c) => Math.abs(c.timeMs - timeMs) <= MEMORY_MERGE_MS);
  if (near) return { cues, cue: near, added: false };
  const cue: Cue = { slot: MEMORY_SLOT, timeMs: Math.round(timeMs), loopMs: null, color: null, name: '' };
  return { cues: sortCues([...cues, cue]), cue, added: true };
}

export function withoutCue(cues: Cue[], key: string): Cue[] {
  return cues.filter((c) => cueKey(c) !== key);
}

export function withCueName(cues: Cue[], key: string, name: string): Cue[] {
  return cues.map((c) => (cueKey(c) === key ? { ...c, name: name.trim().slice(0, 32) } : c));
}

export function withCueColor(cues: Cue[], key: string, color: string | null): Cue[] {
  return cues.map((c) => (cueKey(c) === key ? { ...c, color } : c));
}

/** Gives the cue `slot` (a pad, A to H) or, with null, takes its pad away. A cue without a pad is a memory cue: removing the pad merges it into a memory cue already within `MEMORY_MERGE_MS`. Null when the pad is taken by another cue, the slot is out of range or the cue is unknown. */
export function withCuePad(cues: Cue[], key: string, slot: number | null): Cue[] | null {
  const cue = cues.find((c) => cueKey(c) === key);
  if (!cue) return null;
  const rest = withoutCue(cues, key);
  if (slot === null) {
    if (cue.slot === MEMORY_SLOT) return cues;
    const near = memoryCues(rest).some((c) => Math.abs(c.timeMs - cue.timeMs) <= MEMORY_MERGE_MS);
    return near ? rest : sortCues([...rest, { ...cue, slot: MEMORY_SLOT }]);
  }
  if (slot < 1 || slot > HOT_CUE_SLOTS || hotCueIn(rest, slot)) return null;
  return sortCues([...rest, { ...cue, slot }]);
}

export interface CueChip {
  key: string;
  slot: number;
  padded: boolean;
  letter: string;
  color: string;
  time: string;
  timeMs: number;
  name: string;
  label: string;
}

export function cueChip(cue: Cue): CueChip {
  const letter = cue.slot === MEMORY_SLOT ? '' : slotLetter(cue.slot);
  const time = formatCueTime(cue.timeMs);
  return {
    key: cueKey(cue),
    slot: cue.slot,
    padded: cue.slot !== MEMORY_SLOT,
    letter,
    color: cueColor(cue),
    time,
    timeMs: cue.timeMs,
    name: cue.name,
    label: [letter, cue.name, time].filter(Boolean).join(' '),
  };
}

export interface CueMarker {
  key: string;
  position: number;
  color: string;
  timeMs: number;
  label: string;
  padded: boolean;
}

export function cueMarkers(cues: Cue[], durationMs: number): CueMarker[] {
  if (!durationMs) return [];
  return cues.map((c) => ({
    key: cueKey(c),
    position: Math.max(0, Math.min(1, c.timeMs / durationMs)),
    color: cueColor(c),
    timeMs: c.timeMs,
    label: c.slot === MEMORY_SLOT ? '' : slotLetter(c.slot),
    padded: c.slot !== MEMORY_SLOT,
  }));
}

/** Where the CUE button returns to on a freshly loaded track: the first memory cue, else the start. */
export function initialCuePointMs(cues: Cue[]): number {
  return memoryCues(cues)[0]?.timeMs ?? 0;
}

export function sameCues(a: Cue[], b: Cue[]): boolean {
  return a.length === b.length && a.every((c, i) => c.slot === b[i].slot && c.timeMs === b[i].timeMs && c.loopMs === b[i].loopMs && c.color === b[i].color && c.name === b[i].name);
}
