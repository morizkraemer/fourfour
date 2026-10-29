import { describe, expect, it, vi } from 'vitest';
import {
  duplicatesTitle,
  importSummary,
  importWithDuplicateCheck,
  playlistImportMessage,
  trackTime,
  type ImportOutcome,
} from './importDuplicates.ts';
import type { DuplicateRow, ImportPlan, SkippedFiles, TrackInfo } from '../services/tauri.svelte.ts';

const row: DuplicateRow = {
  new_path: '/New/glass_harbor.mp3',
  file_name: 'glass_harbor.mp3',
  existing_id: 7,
  title: 'Glass Harbor',
  artist: 'Ode & Ash',
  duration_secs: 425,
  reason: 'same_tags',
};

const NONE_SKIPPED: SkippedFiles = { unsupported: 0, unsupported_exts: [], unreadable: 0 };

function steps(plan: ImportPlan, skip: boolean) {
  return {
    plan: vi.fn(async () => plan),
    ask: vi.fn(async () => skip),
    commit: vi.fn(async (_token: number, skipDuplicates: boolean) => ({ added: [], skipped_ids: skipDuplicates ? [7] : [] })),
  };
}

describe('importWithDuplicateCheck', () => {
  it('adds right away without asking when nothing looks like a duplicate', async () => {
    const s = steps({ token: 4, new_count: 2, already_in_library: 1, duplicates: [], skipped: NONE_SKIPPED }, false);
    await importWithDuplicateCheck(['/New'], s);
    expect(s.ask).not.toHaveBeenCalled();
    expect(s.commit).toHaveBeenCalledWith(4, true);
  });

  it.each([true, false])('passes the answer (skip = %s) to the commit', async (skip) => {
    const s = steps({ token: 9, new_count: 1, already_in_library: 0, duplicates: [row], skipped: NONE_SKIPPED }, skip);
    const result = await importWithDuplicateCheck(['/New'], s);
    expect(s.ask).toHaveBeenCalledWith([row]);
    expect(s.commit).toHaveBeenCalledWith(9, skip);
    expect(result.skipped_ids).toEqual(skip ? [7] : []);
  });
});

describe('duplicate dialog copy', () => {
  it('counts tracks in the title', () => {
    expect(duplicatesTitle(1)).toBe('1 track looks like one you already have');
    expect(duplicatesTitle(12)).toBe('12 tracks look like ones you already have');
  });

  it('formats track time as m:ss', () => {
    expect(trackTime(425)).toBe('7:05');
    expect(trackTime(59.6)).toBe('1:00');
    expect(trackTime(0)).toBe('—');
  });

});

function outcome(added: number, known: number, extra: Partial<SkippedFiles> = {}, skippedIds: number[] = []): ImportOutcome {
  return {
    added: Array.from({ length: added }, (_, i) => ({ id: 100 + i }) as TrackInfo),
    skipped_ids: skippedIds,
    plan: {
      token: 1,
      new_count: added,
      already_in_library: known,
      duplicates: [],
      skipped: { ...NONE_SKIPPED, ...extra },
    },
  };
}

describe('import status copy', () => {
  it('says when a playlist drop used tracks already in the library', () => {
    expect(playlistImportMessage(outcome(0, 12), { kind: 'added', name: 'Warmup', tracks: 12, addedToPlaylist: 12 })).toBe(
      'Added 12 tracks to ‘Warmup’ (already in library).'
    );
    expect(playlistImportMessage(outcome(5, 7), { kind: 'added', name: 'Warmup', tracks: 12, addedToPlaylist: 12 })).toBe(
      'Added 12 tracks to ‘Warmup’ (5 new, 7 already in library).'
    );
    expect(playlistImportMessage(outcome(3, 0), { kind: 'added', name: 'Warmup', tracks: 3, addedToPlaylist: 3 })).toBe(
      'Added 3 tracks to ‘Warmup’.'
    );
    expect(playlistImportMessage(outcome(0, 4), { kind: 'added', name: 'Warmup', tracks: 4, addedToPlaylist: 0 })).toBe(
      'All 4 tracks are already in ‘Warmup’.'
    );
  });

  it('names created playlists', () => {
    expect(playlistImportMessage(outcome(8, 0), { kind: 'created', names: ['Set One'], tracks: 8 })).toBe(
      'Created ‘Set One’ with 8 tracks.'
    );
    expect(
      playlistImportMessage(outcome(2, 3, { unsupported: 1, unsupported_exts: ['.txt'] }), {
        kind: 'created',
        names: ['A', 'B'],
        tracks: 5,
      })
    ).toBe('Created 2 playlists (‘A’, ‘B’) with 5 tracks (2 new, 3 already in library) · 1 file skipped: unsupported format (.txt).');
  });
});

describe('import activity summary', () => {
  it('leads with what was added and what was skipped', () => {
    expect(importSummary(outcome(12, 0, {}, [1, 2, 3])).headline).toBe('Added 12 · 3 duplicates skipped');
  });

  it('says plainly when everything is already in the library', () => {
    expect(importSummary(outcome(0, 12)).headline).toBe('All 12 files already in your library');
    expect(importSummary(outcome(0, 1)).headline).toBe('File already in your library');
    expect(importSummary(outcome(0, 0)).headline).toBe('No audio files found');
  });

  it('counts files that were not imported in the headline', () => {
    expect(importSummary(outcome(2, 0, { unsupported: 3, unsupported_exts: ['.m4p'], unreadable: 1 })).headline).toBe(
      'Added 2 · 4 files not imported',
    );
  });

  it('lists every outcome on its own line with extensions', () => {
    const summary = importSummary(
      outcome(5, 7, { unsupported: 3, unsupported_exts: ['.m4p', '.txt'], unreadable: 1 }, [9]),
      'Added 5 tracks to ‘Warmup’.',
    );
    expect(summary.lines).toEqual([
      'Added 5 tracks to ‘Warmup’.',
      'Added 5 tracks',
      '7 already in your library',
      '1 duplicate skipped',
      '3 files skipped: unsupported format (.m4p, .txt)',
      '1 audio file couldn’t be read',
    ]);
  });
});
