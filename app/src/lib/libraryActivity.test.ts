import { describe, expect, it } from 'vitest';
import { libraryActivity, type LibraryActivityInput } from './libraryActivity.ts';

const quiet: LibraryActivityInput = {
  importing: false,
  importDone: 0,
  importTotal: 0,
  resultHeadline: null,
  resultFailed: false,
  problems: '',
};

describe('libraryActivity', () => {
  it('rests as an import button when there is nothing to show', () => {
    expect(libraryActivity(quiet)).toEqual({ kind: 'rest', label: 'Import…' });
  });

  it('shows file problems when nothing else is happening', () => {
    expect(libraryActivity({ ...quiet, problems: '3 missing' })).toEqual({ kind: 'problems', label: '3 missing' });
  });

  it('shows the import result over file problems', () => {
    expect(libraryActivity({ ...quiet, problems: '3 missing', resultHeadline: 'Added 12' })).toEqual({
      kind: 'result',
      label: 'Added 12',
      failed: false,
    });
  });

  it('shows counted progress over everything else', () => {
    expect(
      libraryActivity({ ...quiet, importing: true, importDone: 12, importTotal: 40, resultHeadline: 'Added 12', problems: '3 missing' }),
    ).toEqual({ kind: 'importing', label: 'Importing 12/40…', fraction: 0.3 });
  });

  it('shows indeterminate progress until the total is known', () => {
    expect(libraryActivity({ ...quiet, importing: true })).toEqual({ kind: 'importing', label: 'Importing…', fraction: null });
  });
});
