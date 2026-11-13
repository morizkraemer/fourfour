import { beforeEach, describe, expect, it, vi } from 'vitest';

const hooks = vi.hoisted(() => ({ loadedId: null as number | null, order: [] as string[] }));

vi.mock('../services/tauri.svelte.ts', () => ({ setCues: async () => {} }));
vi.mock('./player.svelte.ts', () => ({
  player: {
    get track() {
      return hooks.loadedId === null ? null : { id: hooks.loadedId };
    },
  },
  currentPositionMs: () => 12345.4,
  loadPlayerTrack: vi.fn(async (track: { id: number }) => {
    hooks.loadedId = track.id;
  }),
  seekToMs: vi.fn(async (ms: number) => {
    hooks.order.push(`seek ${ms}`);
  }),
  seekToProgress: vi.fn(),
  seekToMsAndPlay: vi.fn(async (ms: number) => {
    hooks.order.push(`seek+play ${ms}`);
  }),
}));

import { cuesOf, loadCuesFromPayload } from './cues.svelte.ts';
import { jumpToInspectorCue, pressInspectorPad } from './inspector-pads.ts';

let track = 500;

beforeEach(() => {
  track += 1;
  hooks.loadedId = null;
  hooks.order = [];
  loadCuesFromPayload(track, []);
});

describe('inspector cue pads', () => {
  it('saves a cue at the playhead when an empty pad of the loaded track is tapped', async () => {
    hooks.loadedId = track;
    await pressInspectorPad({ id: track }, 3, false);
    expect(cuesOf(track).map((c) => [c.slot, c.timeMs])).toEqual([[3, 12345]]);
  });

  it('does nothing on an empty pad of a track that is not loaded', async () => {
    hooks.loadedId = track + 1000;
    await pressInspectorPad({ id: track }, 1, false);
    expect(cuesOf(track)).toEqual([]);
  });

  it('jumps to a set pad of the loaded track and plays, keeping the cue', async () => {
    hooks.loadedId = track;
    loadCuesFromPayload(track, [{ hot_cue_number: 2, time_ms: 9000 }]);
    await pressInspectorPad({ id: track }, 2, false);
    expect(hooks.order).toEqual(['seek+play 9000']);
    expect(cuesOf(track)).toHaveLength(1);
  });

  it('loads and only jumps on a set pad of a track that is not loaded', async () => {
    hooks.loadedId = track + 1000;
    loadCuesFromPayload(track, [{ hot_cue_number: 2, time_ms: 9000 }]);
    await pressInspectorPad({ id: track }, 2, false);
    expect(hooks.loadedId).toBe(track);
    expect(hooks.order).toEqual(['seek 9000']);
  });

  it('deletes a set pad with Alt held, on any track', async () => {
    loadCuesFromPayload(track, [{ hot_cue_number: 2, time_ms: 9000 }, { hot_cue_number: 0, time_ms: 100 }]);
    await pressInspectorPad({ id: track }, 2, true);
    expect(cuesOf(track).map((c) => c.slot)).toEqual([0]);
  });

  it('keeps an empty pad empty with Alt held and only sets it', async () => {
    hooks.loadedId = track;
    await pressInspectorPad({ id: track }, 1, true);
    expect(cuesOf(track).map((c) => c.slot)).toEqual([1]);
  });
});

describe('inspector memory cue chips', () => {
  it('fires a memory cue of the loaded track: jump and play', async () => {
    hooks.loadedId = track;
    await jumpToInspectorCue({ id: track }, { timeMs: 14000, padded: false });
    expect(hooks.order).toEqual(['seek+play 14000']);
  });

  it('loads and only jumps for a memory cue of another track', async () => {
    hooks.loadedId = track + 1000;
    await jumpToInspectorCue({ id: track }, { timeMs: 14000, padded: false });
    expect(hooks.order).toEqual(['seek 14000']);
  });

  it('only jumps for a hot cue chip', async () => {
    hooks.loadedId = track;
    await jumpToInspectorCue({ id: track }, { timeMs: 9000, padded: true });
    expect(hooks.order).toEqual(['seek 9000']);
  });
});
