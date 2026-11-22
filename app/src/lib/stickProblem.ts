import type { StickStatusKind } from './stickStatus.ts';

export type StickProblemKind = Extract<StickStatusKind, 'error' | 'wont-fit' | 'unanalysed'>;

export interface StickProblemPlaylist {
  id: number;
  name: string;
  bytes: number;
}

export interface StickProblemInput {
  kind: StickProblemKind;
  stickName: string;
  bytesNeeded: number;
  freeBytes: number | null;
  playlists: StickProblemPlaylist[];
  unanalysed: { title: string; artist: string }[];
  error: string;
}

export interface StickProblem {
  kind: StickProblemKind;
  title: string;
  what: string;
  todo: string;
  playlists: StickProblemPlaylist[];
  names: string[];
  moreNames: number;
  action: 'retry' | 'analyze' | null;
}

const MAX_NAMES = 4;
const MB = 1_000_000;

export function roundBytes(bytes: number): string {
  if (bytes >= 1000 * MB) return `${(bytes / (1000 * MB)).toFixed(1)} GB`;
  if (bytes >= MB) return `${Math.round(bytes / MB)} MB`;
  return `${Math.max(1, Math.round(bytes / 1000))} KB`;
}

export function isStickProblem(kind: StickStatusKind): kind is StickProblemKind {
  return kind === 'error' || kind === 'wont-fit' || kind === 'unanalysed';
}

function trackNames(tracks: { title: string; artist: string }[]): string[] {
  return tracks.slice(0, MAX_NAMES).map((t) => (t.artist ? `${t.artist} - ${t.title}` : t.title));
}

export function stickProblem(input: StickProblemInput): StickProblem {
  const base = { kind: input.kind, playlists: [], names: [], moreNames: 0, action: null };
  if (input.kind === 'wont-fit') {
    const free = input.freeBytes ?? 0;
    const over = Math.max(0, input.bytesNeeded - free);
    return {
      ...base,
      title: `Too much for ${input.stickName}`,
      what: `${roundBytes(over)} too much: the new tracks need ${roundBytes(input.bytesNeeded)}, the stick has ${roundBytes(free)} free.`,
      todo: input.playlists.length > 0 ? 'Take a playlist off, or free up space.' : 'Remove tracks from it, or free up space.',
      playlists: [...input.playlists].sort((a, b) => b.bytes - a.bytes),
    };
  }
  if (input.kind === 'unanalysed') {
    const count = input.unanalysed.length;
    return {
      ...base,
      title: count === 1 ? '1 track is not analyzed' : `${count} tracks are not analyzed`,
      what: `${count === 1 ? 'It is' : 'They are'} left off ${input.stickName} until analyzed. Everything else syncs as usual.`,
      todo: '',
      names: trackNames(input.unanalysed),
      moreNames: Math.max(0, count - MAX_NAMES),
      action: 'analyze',
    };
  }
  return {
    ...base,
    title: `Sync to ${input.stickName} failed`,
    what: input.error || 'The sync stopped. Check the stick, then sync again.',
    todo: '',
    action: 'retry',
  };
}
