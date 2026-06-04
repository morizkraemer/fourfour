/*
 * Camelot key → color token. The wheel number (1-12) picks the hue; the A/B
 * suffix shares a hue. Returns a `var(--ff-key-N)` reference (or muted for
 * unknown / unanalyzed keys like "—"). One source for track-row + detail pane.
 */
export function keyColor(key) {
  const m = /^(\d{1,2})[AB]$/i.exec(String(key ?? '').trim());
  if (!m) return 'var(--ff-muted)';
  const n = Number(m[1]);
  if (n < 1 || n > 12) return 'var(--ff-muted)';
  return `var(--ff-key-${n})`;
}
