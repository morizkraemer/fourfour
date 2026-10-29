import { describe, expect, it, vi } from 'vitest';
import { duplicatesTitle, importSummary, importWithDuplicateCheck, trackTime } from './importDuplicates.ts';
import type { DuplicateRow, ImportPlan } from '../services/tauri.svelte.ts';

const row: DuplicateRow = {
  new_path: '/New/glass_harbor.mp3',
  file_name: 'glass_harbor.mp3',
  existing_id: 7,
  title: 'Glass Harbor',
  artist: 'Ode & Ash',
  duration_secs: 425,
  reason: 'same_tags',
};

function steps(plan: ImportPlan, skip: boolean) {
  return {
    plan: vi.fn(async () => plan),
    ask: vi.fn(async () => skip),
    commit: vi.fn(async (_token: number, skipDuplicates: boolean) => ({ added: [], skipped_ids: skipDuplicates ? [7] : [] })),
  };
}

describe('importWithDuplicateCheck', () => {
  it('adds right away without asking when nothing looks like a duplicate', async () => {
    const s = steps({ token: 4, new_count: 2, already_in_library: 1, duplicates: [] }, false);
    await importWithDuplicateCheck(['/New'], s);
    expect(s.ask).not.toHaveBeenCalled();
    expect(s.commit).toHaveBeenCalledWith(4, true);
  });

  it.each([true, false])('passes the answer (skip = %s) to the commit', async (skip) => {
    const s = steps({ token: 9, new_count: 1, already_in_library: 0, duplicates: [row] }, skip);
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

  it('summarises added and skipped tracks', () => {
    expect(importSummary(3, 0)).toBe('Imported 3 tracks');
    expect(importSummary(1, 2)).toBe('Imported 1 track, skipped 2 duplicates');
    expect(importSummary(0, 1)).toBe('Skipped 1 duplicate');
  });
});
