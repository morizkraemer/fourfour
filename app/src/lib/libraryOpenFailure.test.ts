import { describe, expect, it } from 'vitest';
import { CommandError, type ErrorKind } from '../services/command-error.ts';
import { openFailureKind, parseOpenFailure, recoveryFailureText } from './libraryOpenFailure.ts';

const DB = '/data/library.db';
const failure = (kind: ErrorKind, message = 'Anything the backend says', path: string | null = DB) => new CommandError(kind, message, path);
const failed = (kind: ErrorKind) => failure(kind, `Couldn't open the library at ${DB}: reworded`);

describe('openFailureKind', () => {
  it('reads the cause from the error kind, whatever the message says', () => {
    expect(openFailureKind(failure('library_newer_version', 'rewritten'))).toBe('newer');
    expect(openFailureKind(failure('library_locked', 'rewritten'))).toBe('locked');
    expect(openFailureKind(failure('library_damaged', 'rewritten'))).toBe('damaged');
    expect(openFailureKind(failure('permission_denied'))).toBe('unknown');
    expect(openFailureKind(failure('other'))).toBe('unknown');
    expect(openFailureKind(failure('library_not_open', 'The library is not open', null))).toBe('unknown');
  });

  it('ignores errors that are not about opening the library', () => {
    expect(openFailureKind(failure('other', 'poisoned lock: another task failed inside', null))).toBeNull();
    expect(openFailureKind(failure('not_found', 'Track 4 is not in the library', null))).toBeNull();
    expect(openFailureKind(failure('library_locked', 'busy while saving', null))).toBeNull();
    expect(openFailureKind("Couldn't open the library at /data/library.db: database is locked")).toBeNull();
    expect(openFailureKind(new Error('a newer version'))).toBeNull();
    expect(parseOpenFailure('database is locked')).toBeNull();
  });
});

describe('parseOpenFailure', () => {
  it('offers Try again only for a library that is in use', () => {
    expect(parseOpenFailure(failed('library_locked'))?.canRetry).toBe(true);
    expect(parseOpenFailure(failed('library_damaged'))?.canRetry).toBe(false);
    expect(parseOpenFailure(failed('library_newer_version'))?.canRetry).toBe(false);
  });

  it('says what happened and what to do, in plain words', () => {
    for (const kind of ['library_newer_version', 'library_locked', 'library_damaged', 'other'] as const) {
      const open = parseOpenFailure(failed(kind))!;
      const copy = `${open.title} ${open.text}`;
      expect(copy).not.toMatch(/[–—!]/);
      expect(copy).not.toMatch(/schema|sqlite|database|error|library\.db|\//i);
      expect(open.text.length).toBeGreaterThan(40);
    }
    expect(parseOpenFailure(failed('library_damaged'))?.text).toContain('has not been changed');
  });
});

describe('recoveryFailureText', () => {
  it('rewords open errors without internals and passes plain messages through', () => {
    expect(recoveryFailureText(failed('library_damaged'))).toBe("fourfour can't read that library file. It has not been changed.");
    expect(recoveryFailureText(failed('library_locked'))).toContain('in use');
    expect(recoveryFailureText(failed('library_newer_version'))).toContain('Update fourfour');
    expect(recoveryFailureText(failure('not_found', 'There is no fourfour library in that folder. Choose the folder that holds your library.', null))).toBe(
      'There is no fourfour library in that folder. Choose the folder that holds your library.',
    );
  });
});
