import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), save: vi.fn() }));

import {
  CommandError,
  ERROR_KINDS,
  commandErrorKind,
  commandErrorPath,
  invoke,
  toCommandError,
} from './command-error.ts';
import { changeLibraryPath, loadState, readUsbState } from './tauri.svelte.ts';

const WIRE_MESSAGES: Array<[string, string]> = [
  ['library_newer_version', 'Couldn\'t open the library at /lib/library.db: The library was saved by a newer version of the app (schema version 99, this app knows up to 8); update the app to open it'],
  ['library_damaged', "Couldn't open the library at /lib/library.db: file is not a database"],
  ['library_locked', 'database is locked'],
  ['not_found', 'File not found: /music/a.mp3'],
  ['permission_denied', 'Failed to create /x/y.partial: Permission denied (os error 13)'],
  ['disk_full', 'No space left on device (os error 28)'],
  ['no_space_on_stick', 'Not enough space on STICK: needs 2.0 GB, 1.0 GB free. Nothing was written.'],
  ['drive_gone', 'Cannot open /Volumes/STICK: No such file or directory (os error 2)'],
  ['stick_busy', 'STICK is already being synced.'],
  ['decode_failed', 'Unsupported or corrupt audio file /music/a.mp3: end of stream'],
  ['audio_output_unavailable', 'Audio output error: device unplugged'],
  ['models_missing', 'No analysis backend available: the Rust model weights (beat_this.onnx, mel_spectrogram.onnx) were not found'],
  ['analysis_failed', 'Analysis failed for all 3 tracks — see debug log'],
  ['invalid_backup', 'This file is not a library backup'],
  ['other', 'poisoned lock: another task failed inside'],
];

describe('a typed backend failure', () => {
  it('keeps the exact text for String(err), template strings and err.message', () => {
    for (const [kind, message] of WIRE_MESSAGES) {
      const err = toCommandError({ kind, message });
      expect(String(err)).toBe(message);
      expect(`${err}`).toBe(message);
      expect(err instanceof Error ? err.message : String(err)).toBe(message);
    }
  });

  it('covers every kind the backend sends', () => {
    expect(WIRE_MESSAGES.map(([kind]) => kind).sort()).toEqual([...ERROR_KINDS].sort());
  });

  it('exposes the kind and the path to new callers', () => {
    const err = toCommandError({ kind: 'library_locked', message: 'x', path: '/lib/library.db' });
    expect(err).toBeInstanceOf(CommandError);
    expect(commandErrorKind(err)).toBe('library_locked');
    expect(commandErrorPath(err)).toBe('/lib/library.db');
    expect(commandErrorPath(toCommandError({ kind: 'other', message: 'x' }))).toBeNull();
  });

  it('reads a kind it does not know as other', () => {
    expect(commandErrorKind(toCommandError({ kind: 'from_a_newer_backend', message: 'x' }))).toBe('other');
  });
});

describe('what is not a typed failure', () => {
  it('passes strings, errors and other values through untouched', () => {
    const error = new Error('boom');
    expect(toCommandError('state not managed for field `engine`')).toBe('state not managed for field `engine`');
    expect(toCommandError(error)).toBe(error);
    expect(toCommandError(null)).toBeNull();
    expect(toCommandError({ message: 'no kind' })).toEqual({ message: 'no kind' });
    expect(toCommandError({ kind: 3, message: 'kind is not text' })).toEqual({ kind: 3, message: 'kind is not text' });
  });

  it('has no kind when the failure was a string', () => {
    expect(commandErrorKind('plain text')).toBeNull();
    expect(commandErrorKind(new Error('boom'))).toBeNull();
    expect(commandErrorKind(undefined)).toBeNull();
  });
});

describe('invoke', () => {
  afterEach(() => invokeMock.mockReset());

  it('resolves with what the command returned and forwards the arguments unchanged', async () => {
    invokeMock.mockResolvedValue(7);
    await expect(invoke<number>('count', { a: 1 })).resolves.toBe(7);
    expect(invokeMock).toHaveBeenCalledWith('count', { a: 1 });
    await invoke('plain');
    expect(invokeMock).toHaveBeenLastCalledWith('plain');
  });

  it('rejects with a CommandError for a typed failure', async () => {
    invokeMock.mockRejectedValue({ kind: 'not_found', message: 'File not found: /a.mp3', path: '/a.mp3' });
    const err = await invoke('reveal_in_finder', { path: '/a.mp3' }).catch((e: unknown) => e);
    expect(commandErrorKind(err)).toBe('not_found');
    expect(String(err)).toBe('File not found: /a.mp3');
  });

  it('rejects with the same string for a string failure', async () => {
    invokeMock.mockRejectedValue('The library is not open');
    const err = await invoke('load_state').catch((e: unknown) => e);
    expect(err).toBe('The library is not open');
  });
});

describe('the service wrappers inside the real app', () => {
  beforeEach(() => {
    vi.stubGlobal('window', { __TAURI_INTERNALS__: {} });
  });
  afterEach(() => {
    vi.unstubAllGlobals();
    invokeMock.mockReset();
  });

  it('loadState rejects with the text the backend sent and its kind', async () => {
    const message = "Couldn't open the library at /lib/library.db: file is not a database";
    invokeMock.mockRejectedValue({ kind: 'library_damaged', message, path: '/lib/library.db' });
    const err = await loadState().catch((e: unknown) => e);
    expect(String(err)).toBe(message);
    expect(commandErrorKind(err)).toBe('library_damaged');
    expect(commandErrorPath(err)).toBe('/lib/library.db');
  });

  it('changeLibraryPath rejects with the same string as before', async () => {
    invokeMock.mockRejectedValue({ kind: 'permission_denied', message: 'unable to open database file' });
    await expect(changeLibraryPath('/music')).rejects.toThrow('unable to open database file');
    const err = await changeLibraryPath('/music').catch((e: unknown) => e);
    expect(String(err)).toBe('unable to open database file');
  });

  it('readUsbState still rejects with a string-like failure for a string rejection', async () => {
    invokeMock.mockRejectedValue('Cannot open /media/usb');
    const err = await readUsbState('/media/usb').catch((e: unknown) => e);
    expect(String(err)).toBe('Cannot open /media/usb');
    expect(commandErrorKind(err)).toBeNull();
  });
});
