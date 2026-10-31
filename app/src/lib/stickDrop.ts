export type StickDropSource =
  | { kind: 'playlist'; fromStick: boolean; alreadyThere: boolean }
  | { kind: 'tracks'; count: number };

/** The one line a stick drop target shows while something hovers it. */
export function stickDropLabel(stickName: string, source: StickDropSource): string {
  if (source.kind === 'tracks') {
    return `Add ${source.count} ${source.count === 1 ? 'track' : 'tracks'} to ${stickName}`;
  }
  if (source.alreadyThere) return `Already on ${stickName}`;
  return `${source.fromStick ? 'Copy to' : 'Put on'} ${stickName} · Keep updated`;
}

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** "just now", "2 min ago", "3 h ago", "yesterday", or a short date. */
export function syncAge(iso: string, now: number = Date.now()): string {
  const at = new Date(iso).getTime();
  if (Number.isNaN(at)) return '';
  const ago = Math.max(0, now - at);
  if (ago < MINUTE) return 'just now';
  if (ago < HOUR) return `${Math.floor(ago / MINUTE)} min ago`;
  if (ago < DAY) return `${Math.floor(ago / HOUR)} h ago`;
  if (ago < 2 * DAY) return 'yesterday';
  return shortDate(iso);
}

/** "Oct 3" */
export function shortDate(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return '';
  return date.toLocaleDateString('en', { month: 'short', day: 'numeric' });
}
