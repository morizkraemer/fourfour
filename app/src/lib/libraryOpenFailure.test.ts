import { describe, expect, it } from 'vitest';
import { openFailureKind, parseOpenFailure, recoveryFailureText } from './libraryOpenFailure.ts';

const at = (reason: string) => `Couldn't open the library at /data/library.db: ${reason}`;

describe('openFailureKind', () => {
  it('sorts the startup messages by cause', () => {
    expect(
      openFailureKind(at('The library was saved by a newer version of the app (schema version 99, this app knows up to 7); update the app to open it')),
    ).toBe('newer');
    expect(openFailureKind(at("The library's schema version is unreadable; it was not opened so nothing in it is changed: database is locked"))).toBe('locked');
    expect(openFailureKind(at('file is not a database'))).toBe('damaged');
    expect(openFailureKind(at('database disk image is malformed'))).toBe('damaged');
    expect(openFailureKind(at('The library has no schema version row; it was not opened so nothing in it is changed'))).toBe('damaged');
    expect(openFailureKind(at('something nobody planned for'))).toBe('unknown');
    expect(openFailureKind("Couldn't find where the library is stored: no app data dir")).toBe('unknown');
  });

  it('ignores errors that are not about opening the library', () => {
    expect(openFailureKind('poisoned lock: another task failed inside')).toBeNull();
    expect(openFailureKind('Track 4 is not in the library')).toBeNull();
    expect(parseOpenFailure('database is locked')).toBeNull();
  });
});

describe('parseOpenFailure', () => {
  it('offers Try again only for a library that is in use', () => {
    expect(parseOpenFailure(at('database is locked'))?.canRetry).toBe(true);
    expect(parseOpenFailure(at('file is not a database'))?.canRetry).toBe(false);
    expect(parseOpenFailure(at('The library was saved by a newer version of the app'))?.canRetry).toBe(false);
  });

  it('says what happened and what to do, in plain words', () => {
    for (const reason of ['a newer version of the app', 'database is locked', 'file is not a database', 'something else']) {
      const failure = parseOpenFailure(at(reason))!;
      const copy = `${failure.title} ${failure.text}`;
      expect(copy).not.toMatch(/[–—!]/);
      expect(copy).not.toMatch(/schema|sqlite|database|error|library\.db|\//i);
      expect(failure.text.length).toBeGreaterThan(40);
    }
    expect(parseOpenFailure(at('file is not a database'))?.text).toContain('has not been changed');
  });
});

describe('recoveryFailureText', () => {
  it('rewords open errors without internals and passes plain messages through', () => {
    expect(recoveryFailureText(at('file is not a database'))).toBe("fourfour can't read that library file. It has not been changed.");
    expect(recoveryFailureText(at('database is locked'))).toContain('in use');
    expect(recoveryFailureText(at('a newer version of the app'))).toContain('Update fourfour');
    expect(recoveryFailureText('There is no fourfour library in that folder. Choose the folder that holds your library.')).toBe(
      'There is no fourfour library in that folder. Choose the folder that holds your library.',
    );
  });
});
