import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('../services/playback.svelte.ts', () => ({
  playbackPause: vi.fn(),
  playbackPlay: vi.fn(async () => undefined),
  playbackSeek: vi.fn(),
  playbackSetPosition: vi.fn(),
  playbackStop: vi.fn(async () => undefined),
  subscribePlaybackError: vi.fn(async () => () => {}),
  subscribePlaybackTick: vi.fn(async () => () => {}),
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(async () => undefined) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import { invalidateAnalysisCache } from '../services/tauri.svelte.ts';
import { library } from './library.svelte.ts';
import { beatGridNudgeRight, clearPlayerTrack, loadPlayerTrack, player, setExpanded, setTrackMode } from './player.svelte.ts';
import { commitTrackBpm, doubleTrackBpm, halveTrackBpm, stepTrackBpm, stepTrackKey } from './track-edits.svelte.ts';

Object.assign(globalThis, { window: { __TAURI_INTERNALS__: {} } });

const invokeMock = vi.mocked(invoke);

let storedBpm: number;
let storedBeats: Array<{ time_ms: number; bar_position: number }>;
let storedOffsetMs: number;

function backend() {
  invokeMock.mockImplementation(async (command: string, args?: unknown): Promise<any> => {
    if (command === 'get_analysis_data') {
      return {
        waveform_preview: Array.from({ length: 400 }, (_, i) => 1 + (i % 30)),
        bpm: storedBpm,
        key: '1A',
        beats: storedBeats,
        grid_offset_ms: storedOffsetMs,
        duration_ms: 20_000,
      };
    }
    if (command === 'rescale_beat_grid') {
      storedBpm = (args as { bpm: number }).bpm;
    }
    return undefined;
  });
}

const trackInfo = {
  id: 4,
  source_path: '/music/four.wav',
  title: 'Four',
  artist: '',
  tempo: 12000,
  key: '1A',
  duration_secs: 20,
  has_analysis: true,
};

const rescaleCalls = () => invokeMock.mock.calls.filter(([command]) => command === 'rescale_beat_grid');
const row = () => library.tracks[0] as Record<string, any>;

beforeEach(async () => {
  vi.useFakeTimers();
  invalidateAnalysisCache();
  invokeMock.mockReset();
  storedOffsetMs = 0;
  storedBeats = [1, 2, 3, 4, 5].map((n) => ({ time_ms: 500 * n, bar_position: ((n - 1) % 4) + 1 }));
  storedBpm = 120;
  backend();
  await loadPlayerTrack(trackInfo);
  await vi.advanceTimersByTimeAsync(0);
  library.tracks = [{ id: 4, title: 'Four', bpm: '120.0', bpmExact: 120 }] as never;
  invokeMock.mockClear();
});

afterEach(() => vi.useRealTimers());

describe('stepTrackBpm', () => {
  it('steps by 0.1 in the normal tier', () => {
    expect(stepTrackBpm(4, 1, 'normal')).toBe(true);
    expect(row().bpmExact).toBeCloseTo(120.1, 5);
    stepTrackBpm(4, -2, 'normal');
    expect(row().bpmExact).toBeCloseTo(119.9, 5);
  });

  it('steps by 1 in the coarse tier', () => {
    stepTrackBpm(4, 1, 'coarse');
    expect(row().bpmExact).toBeCloseTo(121, 5);
    stepTrackBpm(4, -1, 'coarse');
    stepTrackBpm(4, -1, 'coarse');
    expect(row().bpmExact).toBeCloseTo(119, 5);
  });

  it('steps by 0.01 in the fine tier', () => {
    stepTrackBpm(4, 1, 'fine');
    expect(row().bpmExact).toBeCloseTo(120.01, 5);
    stepTrackBpm(4, -3, 'fine');
    expect(row().bpmExact).toBeCloseTo(119.98, 5);
  });
});

describe('displays', () => {
  it('update the row and the loaded track at once, before anything is saved', () => {
    stepTrackBpm(4, 1, 'normal');
    expect(row().bpm).toBe('120.1');
    expect(row().bpmLocked).toBe(true);
    expect(player.track!.bpm).toBe('120.10');
    expect(rescaleCalls()).toHaveLength(0);
  });

  it('show a typed value', () => {
    expect(commitTrackBpm(4, 127.55)).toBe(true);
    expect(row().bpmExact).toBe(127.55);
    expect(player.track!.bpm).toBe('127.55');
  });
});

describe('saving', () => {
  it('writes one rescale after a burst of steps, with the final value', async () => {
    stepTrackBpm(4, 1, 'normal');
    stepTrackBpm(4, 1, 'normal');
    stepTrackBpm(4, 1, 'coarse');
    stepTrackBpm(4, -1, 'fine');
    await vi.advanceTimersByTimeAsync(399);
    expect(rescaleCalls()).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(10);
    expect(rescaleCalls()).toHaveLength(1);
    expect(rescaleCalls()[0][1]).toEqual({ trackId: 4, bpm: row().bpmExact });
    expect(row().bpmExact).toBeCloseTo(120.99, 5);
  });

  it('restarts the wait with every step', async () => {
    stepTrackBpm(4, 1, 'normal');
    await vi.advanceTimersByTimeAsync(300);
    stepTrackBpm(4, 1, 'normal');
    await vi.advanceTimersByTimeAsync(300);
    expect(rescaleCalls()).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(200);
    expect(rescaleCalls()).toHaveLength(1);
  });

  it('reports a failed save and shows the stored BPM again', async () => {
    invokeMock.mockImplementation(async (command: string): Promise<any> => {
      if (command === 'rescale_beat_grid') throw 'disk full';
      if (command === 'get_analysis_data') return { waveform_preview: Array.from({ length: 400 }, () => 1), bpm: 120, beats: storedBeats, grid_offset_ms: 0, duration_ms: 20_000 };
      return undefined;
    });
    stepTrackBpm(4, 5, 'coarse');
    await vi.advanceTimersByTimeAsync(500);
    expect(library.statusMessage).toBe("Couldn't set the BPM. The disk is full. Free up some space and try again.");
    expect(library.statusMessage).not.toContain('disk full');
    expect(row().bpmExact).toBe(120);
  });
});

describe('double and halve', () => {
  it('doubles and halves the shown BPM', () => {
    commitTrackBpm(4, 100);
    expect(doubleTrackBpm(4)).toBe(true);
    expect(row().bpmExact).toBe(200);
    expect(halveTrackBpm(4)).toBe(true);
    expect(row().bpmExact).toBe(100);
    expect(player.track!.bpm).toBe('100.00');
  });

  it('refuses when the result leaves the range and changes nothing', () => {
    expect(doubleTrackBpm(4)).toBe(true);
    expect(row().bpmExact).toBe(240);
    expect(doubleTrackBpm(4)).toBe(false);
    expect(row().bpmExact).toBe(240);
    commitTrackBpm(4, 60);
    expect(halveTrackBpm(4)).toBe(false);
    expect(row().bpmExact).toBe(60);
  });
});

describe('range limits', () => {
  it('clamps steps at 40 and 250', () => {
    commitTrackBpm(4, 249.5);
    stepTrackBpm(4, 10, 'coarse');
    expect(row().bpmExact).toBe(250);
    commitTrackBpm(4, 40.5);
    stepTrackBpm(4, -10, 'coarse');
    expect(row().bpmExact).toBe(40);
  });

  it('rejects typed values outside the range and non-numbers', () => {
    expect(commitTrackBpm(4, 39.9)).toBe(false);
    expect(commitTrackBpm(4, 250.1)).toBe(false);
    expect(commitTrackBpm(4, NaN)).toBe(false);
    expect(row().bpmExact).toBe(120);
    expect(commitTrackBpm(4, 40)).toBe(true);
    expect(commitTrackBpm(4, 250)).toBe(true);
  });
});

describe('grid reload after the save', () => {
  it('puts the saved grid, with its sub-millisecond offset, into the player', async () => {
    storedBeats = [1, 2, 3, 4, 5].map((n) => ({ time_ms: 400 * n, bar_position: ((n - 1) % 4) + 1 }));
    storedOffsetMs = 0.37;
    stepTrackBpm(4, 1, 'coarse');
    await vi.advanceTimersByTimeAsync(500);
    expect(player.track!.beats.map((b) => b.time_ms)).toEqual([400.37, 800.37, 1200.37, 1600.37, 2000.37]);
    expect(player.track!.beats.map((b) => b.bar_position)).toEqual([1, 2, 3, 4, 1]);
  });

  it('leaves the loaded track alone when another track was edited', async () => {
    library.tracks = [...(library.tracks as any[]), { id: 9, title: 'Nine', bpm: '100.0', bpmExact: 100 }] as never;
    const before = player.track;
    stepTrackBpm(9, 1, 'normal');
    await vi.advanceTimersByTimeAsync(500);
    expect(player.track).toBe(before);
  });
});

describe('grid reload racing newer edits', () => {
  it('keeps a grid nudge made while the BPM save was on its way', async () => {
    let finishRescale!: () => void;
    invokeMock.mockImplementation(async (command: string, args?: unknown): Promise<any> => {
      if (command === 'rescale_beat_grid') {
        storedBpm = (args as { bpm: number }).bpm;
        await new Promise<void>((resolve) => (finishRescale = resolve));
        return undefined;
      }
      if (command === 'get_analysis_data') {
        return { waveform_preview: Array.from({ length: 400 }, () => 1), bpm: storedBpm, beats: storedBeats, grid_offset_ms: 0, duration_ms: 20_000 };
      }
      return undefined;
    });
    stepTrackBpm(4, 1, 'coarse');
    await vi.advanceTimersByTimeAsync(400);
    expect(rescaleCalls()).toHaveLength(1);
    beatGridNudgeRight(5);
    const nudged = player.track!.beats;
    finishRescale();
    await vi.advanceTimersByTimeAsync(0);
    expect(player.track!.beats).toBe(nudged);
  });
});

describe('stepTrackKey', () => {
  it('steps on from the key the previous step asked for while its save is still on its way', async () => {
    library.tracks = [{ id: 4, title: 'Four', bpm: '120.0', bpmExact: 120, key: '8A' }] as never;
    await Promise.all([stepTrackKey(4, 1), stepTrackKey(4, 1)]);
    const keys = invokeMock.mock.calls.filter(([command]) => command === 'set_track_key').map(([, args]) => (args as { key: string }).key);
    expect(keys).toEqual(['9A', '10A']);
    expect(row().key).toBe('10A');
  });

  it('writes the keys one after another so the last step is what stays stored', async () => {
    library.tracks = [{ id: 4, title: 'Four', bpm: '120.0', bpmExact: 120, key: '8A' }] as never;
    const order: string[] = [];
    invokeMock.mockImplementation(async (command: string, args?: unknown): Promise<any> => {
      if (command !== 'set_track_key') return undefined;
      const { key } = args as { key: string };
      await new Promise((resolve) => setTimeout(resolve, key === '9A' ? 50 : 0));
      order.push(key);
      return undefined;
    });
    const steps = Promise.all([stepTrackKey(4, 1), stepTrackKey(4, 1)]);
    await vi.advanceTimersByTimeAsync(100);
    await steps;
    expect(order).toEqual(['9A', '10A']);
    expect(row().key).toBe('10A');
  });
});

describe('track mode', () => {
  it('ends when the player track is cleared', () => {
    setExpanded(true);
    setTrackMode('bpm', true);
    setTrackMode('key', true);
    expect(player.trackBpmMode).toBe(true);
    expect(player.trackKeyMode).toBe(true);
    clearPlayerTrack();
    expect(player.trackBpmMode).toBe(false);
    expect(player.trackKeyMode).toBe(false);
    setExpanded(false);
  });

  it('ends when the big player collapses', () => {
    setExpanded(true);
    setTrackMode('bpm', true);
    setExpanded(false);
    expect(player.trackBpmMode).toBe(false);
  });
});
