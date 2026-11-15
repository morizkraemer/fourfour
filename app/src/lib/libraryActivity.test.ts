import { describe, expect, it } from 'vitest';
import { analysisTimeLeft, libraryActivity, nextAnalysisPace, type LibraryActivityInput } from './libraryActivity.ts';

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
    expect(libraryActivity(busy)).toEqual({ kind: 'analyzing', label: 'Analyzing', detail: '1 / 3', fraction: 1 / 3 });
    expect(libraryActivity({ ...busy, syncing: true })).toEqual({ kind: 'syncing', label: 'Syncing' });
    expect(libraryActivity({ ...busy, syncing: true, importing: true }).kind).toBe('importing');
  });

  it('uses the singular for one track', () => {
    expect(libraryActivity({ ...quiet, trackCount: 1 })).toEqual({ kind: 'rest', label: '1 track' });
  });
});

describe('analysis progress', () => {
  it('fills the bar with the share of tracks analysed, clamped', () => {
    const at = (current: number, total: number) => libraryActivity({ ...quiet, analyzing: true, analysisCurrent: current, analysisTotal: total });
    expect(at(15, 41)).toMatchObject({ fraction: 15 / 41 });
    expect(at(50, 41)).toMatchObject({ fraction: 1 });
    expect(at(0, 0)).toMatchObject({ fraction: 0 });
  });

  it('keeps the pace anchor through a run and starts again when the count drops', () => {
    const first = nextAnalysisPace(null, 1, 1000);
    expect(nextAnalysisPace(first, 5, 9000)).toBe(first);
    expect(nextAnalysisPace(first, 0, 9000)).toEqual({ at: 9000, done: 0 });
  });

  it('estimates the time left only once a few tracks have finished', () => {
    const pace = { at: 0, done: 1 };
    expect(analysisTimeLeft(null, 5, 41, 60_000)).toBeNull();
    expect(analysisTimeLeft(pace, 2, 41, 60_000)).toBeNull();
    expect(analysisTimeLeft(pace, 5, 41, 2000)).toBeNull();
    expect(analysisTimeLeft(pace, 11, 41, 60_000)).toBe('About 3 min left');
    expect(analysisTimeLeft(pace, 11, 12, 60_000)).toBe('Less than a minute left');
    expect(analysisTimeLeft(pace, 11, 1011, 60_000)).toBe('About 1 h 40 min left');
    expect(analysisTimeLeft(pace, 11, 611, 60_000)).toBe('About 1 h left');
    expect(analysisTimeLeft(pace, 41, 41, 60_000)).toBeNull();
  });
});
