import { beforeEach, describe, expect, it, vi } from 'vitest';

const hooks = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: hooks.invoke }));
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

import { library } from './library.svelte.ts';
import { player } from './player.svelte.ts';
import { commitTrackFields } from './track-edits.svelte.ts';

Object.assign(globalThis, { window: { __TAURI_INTERNALS__: {} } });

function seed() {
  library.tracks = [
    { id: 7, title: 'Old', artist: 'A', album: 'Al', genre: 'G', raw: { id: 7, title: 'Old', genre: 'G' } },
    { id: 8, title: 'Other', artist: 'B', album: '—', genre: '—', raw: { id: 8 } },
  ] as never;
  player.track = { id: 7, title: 'Old', artist: 'A' } as never;
}

function row(index: number) {
  return library.tracks[index] as Record<string, any>;
}

describe('commitTrackFields', () => {
  beforeEach(() => {
    hooks.invoke.mockReset();
    seed();
  });

  it('updates the table row, its raw record and the loaded player track from the saved values', async () => {
    hooks.invoke.mockResolvedValue({ title: 'New', artist: 'A', album: 'Al', genre: 'Deep House' });
    expect(await commitTrackFields(7, { title: ' New ', genre: 'Deep House' })).toBe(true);

    expect(hooks.invoke).toHaveBeenCalledWith('update_track_fields', {
      trackId: 7,
      edits: { title: ' New ', genre: 'Deep House' },
    });
    expect([row(0).title, row(0).genre, row(0).raw.genre]).toEqual(['New', 'Deep House', 'Deep House']);
    expect((player.track as Record<string, any>).title).toBe('New');
    expect(row(1).title).toBe('Other');
  });

  it('shows a cleared field as a dash', async () => {
    hooks.invoke.mockResolvedValue({ title: 'Old', artist: '', album: 'Al', genre: 'G' });
    await commitTrackFields(7, { artist: '' });
    expect(row(0).artist).toBe('—');
  });

  it('leaves everything as it was and reports when the backend rejects the edit', async () => {
    hooks.invoke.mockRejectedValue('Title cannot be empty');
    expect(await commitTrackFields(7, { title: ' ' })).toBe(false);

    expect(row(0).title).toBe('Old');
    expect((player.track as Record<string, any>).title).toBe('Old');
    expect(library.statusMessage).toContain('Title cannot be empty');
  });
});
