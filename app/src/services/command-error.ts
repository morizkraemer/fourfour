import { invoke as tauriInvoke, type InvokeArgs, type InvokeOptions } from '@tauri-apps/api/core';

export const ERROR_KINDS = [
  'library_newer_version',
  'library_damaged',
  'library_locked',
  'library_not_open',
  'not_found',
  'permission_denied',
  'disk_full',
  'no_space_on_stick',
  'drive_gone',
  'stick_busy',
  'decode_failed',
  'audio_output_unavailable',
  'models_missing',
  'analysis_failed',
  'invalid_backup',
  'other',
] as const;

export type ErrorKind = (typeof ERROR_KINDS)[number];

export interface CommandErrorWire {
  kind: string;
  message: string;
  path?: string | null;
}

/** A failed backend command. `String(err)` and `err.message` are the text the backend always sent. */
export class CommandError extends Error {
  readonly kind: ErrorKind;
  readonly path: string | null;

  constructor(kind: ErrorKind, message: string, path: string | null = null) {
    super(message);
    this.name = 'CommandError';
    this.kind = kind;
    this.path = path;
  }

  override toString(): string {
    return this.message;
  }
}

function isWire(value: unknown): value is CommandErrorWire {
  if (typeof value !== 'object' || value === null || value instanceof Error) return false;
  const candidate = value as Record<string, unknown>;
  return typeof candidate.kind === 'string' && typeof candidate.message === 'string';
}

function knownKind(kind: string): ErrorKind {
  return (ERROR_KINDS as readonly string[]).includes(kind) ? (kind as ErrorKind) : 'other';
}

/** Turns what a command rejected with into a `CommandError`; plain strings and real errors pass through. */
export function toCommandError(err: unknown): unknown {
  if (!isWire(err)) return err;
  return new CommandError(knownKind(err.kind), err.message, err.path ?? null);
}

/** The kind of a failed command, or `null` when the rejection did not come from a typed command. */
export function commandErrorKind(err: unknown): ErrorKind | null {
  return err instanceof CommandError ? err.kind : null;
}

export function commandErrorPath(err: unknown): string | null {
  return err instanceof CommandError ? err.path : null;
}

export async function invoke<T>(command: string, ...rest: [args?: InvokeArgs, options?: InvokeOptions]): Promise<T> {
  try {
    return await tauriInvoke<T>(command, ...rest);
  } catch (err) {
    throw toCommandError(err);
  }
}
