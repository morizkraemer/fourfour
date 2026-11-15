import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const hooks = vi.hoisted(() => ({
  checkLibraryFiles: vi.fn(),
  getLibraryHealth: vi.fn(),
  listenToLibraryHealth: vi.fn(),
  locateTrackFile: vi.fn(),
  pickDirectory: vi.fn(),
  pickFile: vi.fn(),
  relocateMissingInFolder: vi.fn(),
}));

vi.mock('../services/tauri.svelte.ts', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../services/tauri.svelte.ts')>()),
  ...hooks,
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('../services/playback.svelte.ts', () => ({
  playbackPause: vi.fn(),
  playbackPlay: vi.fn(async () => undefined),
  playbackSeek: vi.fn(),
  playbackSetPosition: vi.fn(),
  playbackStop: vi.fn(async () => undefined),
  subscribePlaybackError: vi.fn(async () => () => {}),
  subscribePlaybackTick: vi.fn(async () => () => {}),
}));

import { RELOCATED_MARK_MS } from '../lib/fileHealth.ts';
import { fileStatus } from './file-status.svelte.ts';
import { library } from './library.svelte.ts';
import { ui } from './ui.svelte.ts';
import {
  applyHealth,
  checkLibraryAgain,
  initFileHealth,
  locateTrack,
  problemTrackIds,
  relocateMissing,
  relocateResult,
  showProblemTracks,
} from './file-health.svelte.ts';

function health(over: Partial<Parameters<typeof applyHealth>[0]> = {}) {
  return {
    running: false,
    checked_at_ms: 1000,
    duration_ms: 12,
    total: 10,
    missing: [] as number[],
    error: [] as number[],
    relocated: [] as number[],
    ...over,
  };
}

function row(index: number) {
  return library.tracks[index] as Record<string, any>;
}

beforeEach(() => {
  vi.useFakeTimers();
  Object.values(hooks).forEach((mock) => mock.mockReset());
  fileStatus.byId = {};
  fileStatus.checkedAt = null;
  fileStatus.onlyProblems = false;
  fileStatus.dialogOpen = false;
  fileStatus.relocating = false;
  relocateResult.report = null;
  relocateResult.error = '';
  library.statusMessage = '';
  library.tracks = [
    { id: 1, filePath: '/old/a.mp3', raw: { source_path: '/old/a.mp3' } },
    { id: 2, filePath: '/old/b.mp3', raw: { source_path: '/old/b.mp3' } },
  ] as never;
});

afterEach(() => {
  vi.runOnlyPendingTimers();
  vi.useRealTimers();
});

describe('applyHealth', () => {
  it('marks missing and unreadable tracks and counts them', () => {
    applyHealth(health({ missing: [1, 2], error: [3] }));
    expect(fileStatus.byId).toEqual({ 1: 'missing', 2: 'missing', 3: 'error' });
    expect([fileStatus.total, fileStatus.missing, fileStatus.error, fileStatus.checkedAt]).toEqual([10, 2, 1, 1000]);
  });

  it('ignores a report older than the one shown, but takes a fresh relink report', () => {
    applyHealth(health({ missing: [1], checked_at_ms: 2000 }));
    applyHealth(health({ missing: [], checked_at_ms: 1500 }));
    expect(fileStatus.byId).toEqual({ 1: 'missing' });
    applyHealth(health({ missing: [], checked_at_ms: 1500 }), [1]);
    expect(fileStatus.byId).toEqual({ 1: 'relocated' });
  });

  it('shows a relinked track as relocated for a few seconds, then clears the mark', () => {
    applyHealth(health(), [5]);
    expect(fileStatus.byId[5]).toBe('relocated');
    vi.advanceTimersByTime(RELOCATED_MARK_MS + 1);
    expect(fileStatus.byId[5]).toBeUndefined();
  });

  it('keeps a relocated mark through a report that arrives before it fades', () => {
    applyHealth(health(), [5]);
    applyHealth(health({ checked_at_ms: 3000 }));
    expect(fileStatus.byId[5]).toBe('relocated');
  });

  it('leaves the problems-only filter once nothing is missing', () => {
    applyHealth(health({ missing: [1], checked_at_ms: 1 }));
    fileStatus.onlyProblems = true;
    applyHealth(health({ checked_at_ms: 2 }));
    expect(fileStatus.onlyProblems).toBe(false);
  });
});

describe('problemTrackIds', () => {
  it('lists missing and unreadable tracks, not relocated ones, unless a state is asked for', () => {
    fileStatus.byId = { 1: 'missing', 2: 'error', 3: 'relocated' };
    expect(problemTrackIds()).toEqual([1, 2]);
    expect(problemTrackIds('relocated')).toEqual([3]);
  });
});

describe('checking and relocating', () => {
  it('reports a failed check in the status line and stops the running marker', async () => {
    hooks.checkLibraryFiles.mockRejectedValue('Cannot read the file list: Input/output error (os error 5)');
    await checkLibraryAgain();
    expect(fileStatus.running).toBe(false);
    expect(library.statusMessage).toBe("Couldn't check your files. The drive stopped responding. Check the connection and try again.");
  });

  it('survives a failing health read at startup', async () => {
    const error = vi.spyOn(console, 'error').mockImplementation(() => {});
    hooks.getLibraryHealth.mockRejectedValue(new Error('boom'));
    await expect(initFileHealth()).resolves.toBeUndefined();
    error.mockRestore();
  });

  it('points moved tracks at their new files and opens the result dialog', async () => {
    hooks.pickDirectory.mockResolvedValue('/new');
    hooks.relocateMissingInFolder.mockResolvedValue({
      report: { matched: [{ track_id: 1, source_path: '/new/a.mp3' }], ambiguous: [2], not_found: [] },
      health: health({ missing: [2], checked_at_ms: 5000 }),
    });

    await relocateMissing();

    expect([row(0).filePath, row(0).raw.source_path]).toEqual(['/new/a.mp3', '/new/a.mp3']);
    expect(row(1).filePath).toBe('/old/b.mp3');
    expect(fileStatus.byId).toEqual({ 1: 'relocated', 2: 'missing' });
    expect([fileStatus.dialogOpen, fileStatus.relocating]).toEqual([true, false]);
  });

  it('does nothing when the folder picker is cancelled', async () => {
    hooks.pickDirectory.mockResolvedValue(null);
    await relocateMissing();
    expect(hooks.relocateMissingInFolder).not.toHaveBeenCalled();
    expect(fileStatus.dialogOpen).toBe(false);
  });

  it('keeps the error and still opens the dialog when relocating fails', async () => {
    hooks.pickDirectory.mockResolvedValue('/new');
    hooks.relocateMissingInFolder.mockRejectedValue('/new is not a folder');
    await relocateMissing();
    expect([relocateResult.error, fileStatus.dialogOpen, fileStatus.relocating]).toEqual([
      "Couldn't relink the files. That isn't a folder. Choose a folder.",
      true,
      false,
    ]);
  });

  it('moves a hand-located track from the open report lists into the matched list', async () => {
    relocateResult.report = { matched: [], ambiguous: [2, 9], not_found: [2, 8] };
    hooks.pickFile.mockResolvedValue('/found/b.mp3');
    hooks.locateTrackFile.mockResolvedValue({
      relinked: { track_id: 2, source_path: '/found/b.mp3' },
      health: health({ checked_at_ms: 6000 }),
    });

    await locateTrack(2);

    expect(relocateResult.report).toEqual({
      matched: [{ track_id: 2, source_path: '/found/b.mp3' }],
      ambiguous: [9],
      not_found: [8],
    });
    expect(row(1).filePath).toBe('/found/b.mp3');
  });
});

describe('showProblemTracks', () => {
  it('filters to problems and jumps to All Tracks', () => {
    ui.activeSidebarRow = 'Something else';
    showProblemTracks();
    expect([fileStatus.onlyProblems, ui.activeSidebarRow]).toEqual([true, 'All Tracks']);
  });
});
