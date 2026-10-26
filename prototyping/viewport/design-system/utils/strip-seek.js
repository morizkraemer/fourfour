/** Track fraction (0..1) under a pointer at `clientX` on a strip that starts at `left` and is `width` wide. */
export function stripFraction(clientX, left, width) {
  if (!(width > 0)) return 0;
  return Math.max(0, Math.min(1, (clientX - left) / width));
}
