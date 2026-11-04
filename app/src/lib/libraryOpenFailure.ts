import { CommandError, type ErrorKind } from '../services/command-error.ts';

export type OpenFailureKind = 'newer' | 'damaged' | 'locked' | 'unknown';

export interface OpenFailure {
  kind: OpenFailureKind;
  title: string;
  text: string;
  canRetry: boolean;
}

const KINDS: Partial<Record<ErrorKind, OpenFailureKind>> = {
  library_newer_version: 'newer',
  library_damaged: 'damaged',
  library_locked: 'locked',
  library_not_open: 'unknown',
};

export function openFailureKind(err: unknown): OpenFailureKind | null {
  if (!(err instanceof CommandError)) return null;
  const kind = KINDS[err.kind];
  if (kind) return err.kind === 'library_not_open' || err.path !== null ? kind : null;
  return err.path !== null ? 'unknown' : null;
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

export function parseOpenFailure(err: unknown): OpenFailure | null {
  const kind = openFailureKind(err);
  return kind ? { kind, ...FAILURES[kind] } : null;
}

const RECOVERY_FAILURES: Record<OpenFailureKind, string> = {
  newer: 'That library was made by a newer version of fourfour. Update fourfour to open it.',
  damaged: "fourfour can't read that library file. It has not been changed.",
  locked: 'That library is in use. Close the other program and try again.',
  unknown: "fourfour can't open that library. It has not been changed.",
};

export function recoveryFailureText(err: unknown): string {
  const kind = openFailureKind(err);
  return kind ? RECOVERY_FAILURES[kind] : err instanceof Error ? err.message : String(err);
}
