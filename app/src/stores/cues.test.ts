import { beforeEach, describe, expect, it, vi } from 'vitest';

const saved: Array<{ trackId: number; cues: unknown[] }> = [];
let failNext = false;

vi.mock('../services/tauri.svelte.ts', () => ({
  setCues: async (trackId: number, cues: unknown[]) => {
    if (failNext) {
      failNext = false;
      throw new Error('disk full');
    }
    saved.push({ trackId, cues });
  },
}));

import { cuesOf, editCues, flushCueEdits, loadCuesFromPayload, onCueSaveError } from './cues.svelte.ts';
import { withHotCue, withMemoryCue, withCueName, withoutCue } from '../lib/cues.ts';

let nextTrack = 100;
let track = nextTrack;

beforeEach(() => {
  saved.length = 0;
  track = ++nextTrack;
});

describe('cue store', () => {
  it('has no cues for an unknown or missing track', () => {
    expect(cuesOf(track)).toEqual([]);
    expect(cuesOf(null)).toEqual([]);
  });

  it('loads the cues of an analysis payload', () => {
    loadCuesFromPayload(track, [{ hot_cue_number: 1, time_ms: 5000 }, { hot_cue_number: 0, time_ms: 100 }]);
    expect(cuesOf(track).map((c) => c.timeMs)).toEqual([5000, 100]);
  });

  it('keeps the same list when the payload says the same', () => {
    loadCuesFromPayload(track, [{ hot_cue_number: 1, time_ms: 5000 }]);
    const before = cuesOf(track);
    loadCuesFromPayload(track, [{ hot_cue_number: 1, time_ms: 5000 }]);
    expect(cuesOf(track)).toBe(before);
  });

  it('shows an edit at once and saves it once the edits stop', async () => {
    loadCuesFromPayload(track, []);
    editCues(track, (cues) => withHotCue(cues, 10_000)!.cues);
    editCues(track, (cues) => withMemoryCue(cues, 20_000).cues);
    expect(cuesOf(track)).toHaveLength(2);
    expect(saved).toHaveLength(0);
    await flushCueEdits();
    expect(saved).toHaveLength(1);
    expect(saved[0].trackId).toBe(track);
    expect(saved[0].cues).toEqual([
      { hot_cue_number: 1, time_ms: 10_000, loop_time_ms: null, color: null, name: '' },
      { hot_cue_number: 0, time_ms: 20_000, loop_time_ms: null, color: null, name: '' },
    ]);
  });

  it('does not save an edit that changes nothing', async () => {
    loadCuesFromPayload(track, []);
    editCues(track, (cues) => cues);
    await flushCueEdits();
    expect(saved).toHaveLength(0);
  });

  it('keeps an unsaved edit when an older analysis payload arrives', async () => {
    loadCuesFromPayload(track, []);
    editCues(track, (cues) => withHotCue(cues, 7000)!.cues);
    loadCuesFromPayload(track, []);
    expect(cuesOf(track)).toHaveLength(1);
    await flushCueEdits();
    loadCuesFromPayload(track, [{ hot_cue_number: 1, time_ms: 7000, name: 'Saved' }]);
    expect(cuesOf(track)[0].name).toBe('Saved');
  });

  it('writes the waiting edit of one track before another track is edited', async () => {
    const other = track + 1000;
    loadCuesFromPayload(track, []);
    loadCuesFromPayload(other, []);
    editCues(track, (cues) => withHotCue(cues, 1000)!.cues);
    editCues(other, (cues) => withHotCue(cues, 2000)!.cues);
    await flushCueEdits();
    expect(saved.map((s) => s.trackId)).toEqual([track, other]);
  });

  it('rename and delete are edits like any other', async () => {
    loadCuesFromPayload(track, [{ hot_cue_number: 2, time_ms: 3000 }]);
    editCues(track, (cues) => withCueName(cues, 'h2', 'Break'));
    expect(cuesOf(track)[0].name).toBe('Break');
    editCues(track, (cues) => withoutCue(cues, 'h2'));
    expect(cuesOf(track)).toEqual([]);
    await flushCueEdits();
    expect(saved).toHaveLength(1);
    expect(saved[0].cues).toEqual([]);
  });

  it('reports a failed save and keeps the edit on screen', async () => {
    const errors: string[] = [];
    onCueSaveError((message) => errors.push(message));
    loadCuesFromPayload(track, []);
    failNext = true;
    editCues(track, (cues) => withHotCue(cues, 500)!.cues);
    await flushCueEdits();
    expect(errors).toHaveLength(1);
    expect(errors[0]).toContain('disk full');
    expect(cuesOf(track)).toHaveLength(1);
    onCueSaveError(() => undefined);
  });
});
