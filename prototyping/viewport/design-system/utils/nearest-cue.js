export const CUE_HIT_SLACK = 6;

/**
 * The marker closest to a pointer at `clientX` on a strip that starts at `left` and is `width` wide,
 * or null when none lies within `slack` px. Markers sit at `position * width`, kept `edge` px inside the strip.
 */
export function nearestCue(markers, clientX, left, width, slack = CUE_HIT_SLACK, edge = 4) {
  if (!(width > 0)) return null;
  const x = clientX - left;
  let best = null;
  let bestDistance = Infinity;
  for (const marker of markers) {
    const markerX = Math.max(edge, Math.min(width - edge, marker.position * width));
    const distance = Math.abs(markerX - x);
    if (distance <= slack && distance < bestDistance) {
      best = marker;
      bestDistance = distance;
    }
  }
  return best;
}
