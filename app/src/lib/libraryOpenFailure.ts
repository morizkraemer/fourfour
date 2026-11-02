export type OpenFailureKind = 'newer' | 'damaged' | 'locked' | 'unknown';

export interface OpenFailure {
  kind: OpenFailureKind;
  title: string;
  text: string;
  canRetry: boolean;
}

const OPEN_FAILURE_PREFIXES = ["Couldn't open the library", "Couldn't find where the library is stored", 'The library is not open'];

const LOCKED = /database is locked|database table is locked|busy/i;
const DAMAGED = /not a database|malformed|corrupt|unreadable|no schema version|schema version 0|has tables but no|no tables/i;

export function openFailureKind(raw: string): OpenFailureKind | null {
  if (!OPEN_FAILURE_PREFIXES.some((prefix) => raw.startsWith(prefix))) return null;
  if (/newer version/i.test(raw)) return 'newer';
  if (LOCKED.test(raw)) return 'locked';
  if (DAMAGED.test(raw)) return 'damaged';
  return 'unknown';
}

const FAILURES: Record<OpenFailureKind, Omit<OpenFailure, 'kind'>> = {
  newer: {
    title: 'This library was made by a newer version of fourfour',
    text: 'Update fourfour to open it, or choose another option. The library file has not been changed.',
    canRetry: false,
  },
  damaged: {
    title: "Couldn't open your library",
    text: 'The library file is damaged. It has not been changed. Restore a backup, open another library or start a new one.',
    canRetry: false,
  },
  locked: {
    title: 'Your library is in use',
    text: 'Another copy of fourfour or another program has it open. Close it and try again.',
    canRetry: true,
  },
  unknown: {
    title: "Couldn't open your library",
    text: "fourfour can't read the library file. It has not been changed. Choose what to do next.",
    canRetry: false,
  },
};

export function parseOpenFailure(raw: string): OpenFailure | null {
  const kind = openFailureKind(raw);
  return kind ? { kind, ...FAILURES[kind] } : null;
}

const RECOVERY_FAILURES: Record<OpenFailureKind, string> = {
  newer: 'That library was made by a newer version of fourfour. Update fourfour to open it.',
  damaged: "fourfour can't read that library file. It has not been changed.",
  locked: 'That library is in use. Close the other program and try again.',
  unknown: "fourfour can't open that library. It has not been changed.",
};

export function recoveryFailureText(raw: string): string {
  const kind = openFailureKind(raw);
  return kind ? RECOVERY_FAILURES[kind] : raw;
}
