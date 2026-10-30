import { describe, expect, it } from 'vitest';
import { libraryActivity, type LibraryActivityInput } from './libraryActivity.ts';

const quiet: LibraryActivityInput = {
  importing: false,
  importDone: 0,
  importTotal: 0,
  syncing: false,
  analyzing: false,
  analysisCurrent: 0,
  analysisTotal: 0,
  resultHeadline: null,
  resultFailed: false,
  problems: '',
  trackCount: 1234,
};

describe('libraryActivity', () => {
  it('rests as the track count when there is nothing to show', () => {
    expect(libraryActivity(quiet)).toEqual({ kind: 'rest', label: '1,234 tracks' });
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

  it('shows syncing, then analyzing, below importing and above results and problems', () => {
    const busy = { ...quiet, analyzing: true, analysisCurrent: 1, analysisTotal: 3, resultHeadline: 'Added 12', problems: '3 missing' };
    expect(libraryActivity(busy)).toEqual({ kind: 'analyzing', label: 'Analyzing', detail: '1 / 3' });
    expect(libraryActivity({ ...busy, syncing: true })).toEqual({ kind: 'syncing', label: 'Syncing' });
    expect(libraryActivity({ ...busy, syncing: true, importing: true }).kind).toBe('importing');
  });

  it('uses the singular for one track', () => {
    expect(libraryActivity({ ...quiet, trackCount: 1 })).toEqual({ kind: 'rest', label: '1 track' });
  });
});
