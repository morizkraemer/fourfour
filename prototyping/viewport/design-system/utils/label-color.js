/*
 * Rekordbox colour labels. `master.db` and OneLibrary store the colour name,
 * rekordbox.xml stores `#rrggbb`. One source for track-row + sorting.
 */
const LABEL_COLORS = [
  ['pink', '#f472b6'],
  ['red', '#ef4444'],
  ['orange', '#fb923c'],
  ['yellow', '#facc15'],
  ['green', '#4ade80'],
  ['aqua', '#22d3ee'],
  ['blue', '#60a5fa'],
  ['purple', '#a78bfa'],
];

/** CSS colour for a label name or `#rrggbb`, or null when unknown. */
export function labelColor(value) {
  const text = String(value ?? '').trim();
  if (/^#[0-9a-f]{6}$/i.test(text)) return text;
  return LABEL_COLORS.find(([name]) => name === text.toLowerCase())?.[1] ?? null;
}

/** Position in the Rekordbox colour order; other values sort after the named ones. */
export function labelColorRank(value) {
  const index = LABEL_COLORS.findIndex(([name]) => name === String(value ?? '').trim().toLowerCase());
  if (index >= 0) return index;
  return labelColor(value) ? LABEL_COLORS.length : null;
}
