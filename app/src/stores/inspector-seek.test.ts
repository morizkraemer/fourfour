import { beforeEach, describe, expect, it, vi } from 'vitest';

const calls = vi.hoisted(() => ({ order: [] as string[], loadedId: null as number | null }));

vi.mock('./player.svelte.ts', () => ({
  player: {
    get track() {
      return calls.loadedId === null ? null : { id: calls.loadedId };
    },
  },
  loadPlayerTrack: vi.fn(async (track: { id: number }) => {
    calls.order.push(`load ${track.id}`);
    calls.loadedId = track.id;
  }),
  seekToProgress: vi.fn(async (progress: number) => {
    calls.order.push(`seek ${progress}`);
  }),
}));

import { seekTrackToMs, seekTrackToProgress } from './inspector-seek.ts';

beforeEach(() => {
  calls.order = [];
  calls.loadedId = null;
});

describe('inspector seeking', () => {
  it('seeks the loaded track without reloading it', async () => {
    calls.loadedId = 7;
    await seekTrackToProgress({ id: 7 }, 0.4);
    expect(calls.order).toEqual(['seek 0.4']);
  });

  it('loads a track that is not in the player, then seeks', async () => {
    calls.loadedId = 3;
    await seekTrackToProgress({ id: 7 }, 0.25);
    expect(calls.order).toEqual(['load 7', 'seek 0.25']);
  });

  it('turns a cue time into a track fraction', async () => {
    calls.loadedId = 7;
    await seekTrackToMs({ id: 7, raw: { duration_secs: 200 } }, 50_000);
    expect(calls.order).toEqual(['seek 0.25']);
  });

  it('does nothing for a track without a duration', async () => {
    await seekTrackToMs({ id: 7, raw: { duration_secs: 0 } }, 50_000);
    expect(calls.order).toEqual([]);
  });
});
