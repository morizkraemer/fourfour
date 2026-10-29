import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { PlaylistInput, TrackInfo } from '../services/tauri.svelte.ts';

const hooks = vi.hoisted(() => ({
  state: { tracks: [] as unknown[], playlists: [] as unknown[] },
  added: [] as unknown[],
}));

vi.mock('../services/playback.svelte.ts', () => ({
  playbackPause: vi.fn(),
  playbackPlay: vi.fn(async () => undefined),
  playbackSeek: vi.fn(),
  playbackSetPosition: vi.fn(),
  playbackStop: vi.fn(async () => undefined),
  subscribePlaybackError: vi.fn(async () => () => {}),
  subscribePlaybackTick: vi.fn(async () => () => {}),
}));
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (command: string) => {
    if (command === 'load_state') return structuredClone(hooks.state);
    if (command === 'plan_import') {
      return {
        token: 1,
        new_count: hooks.added.length,
        already_in_library: 0,
        duplicates: [],
        skipped: { unsupported: 0, unsupported_exts: [], unreadable: 0 },
      };
    }
    if (command === 'commit_import') return { added: structuredClone(hooks.added), skipped_ids: [] };
    if (command === 'get_app_version') return '0.0.0';
    if (command === 'get_library_path') return '/lib';
    if (command === 'get_mounted_volumes') return [];
    if (command === 'resume_analysis') return 0;
    return undefined;
  }),
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import { importDroppedTracks, importFiles, initLibrary, library, loadState } from './library.svelte.ts';

const BANDS = btoa(String.fromCharCode(...Array.from({ length: 30 }, (_, i) => 10 + i)));

function track(id: number, extra: Partial<TrackInfo> = {}): TrackInfo {
  return {
    id,
    source_path: `/music/t${id}.mp3`,
    title: `Title ${id}`,
    artist: `Artist ${id % 3}`,
    album: 'Album',
    genre: 'House',
    tempo: 0,
    key: '',
    duration_secs: 200 + id,
    bitrate: 320,
    file_size: 1000 + id,
    has_artwork: id % 2 === 0,
    has_cues: false,
    has_analysis: false,
    analysis_pending: false,
    ...extra,
  };
}

function playlist(id: number, name: string, trackIds: number[]): PlaylistInput {
  return { id, name, track_ids: trackIds, parent_id: 0, is_folder: false };
}

function snapshot() {
  return JSON.parse(JSON.stringify(library.tracks));
}

function loadStateCalls() {
  return vi.mocked(invoke).mock.calls.filter(([command]) => command === 'load_state').length;
}

const analysed = { tempo: 12400, key: '8A', has_analysis: true, waveform_bands: BANDS };

beforeEach(async () => {
  hooks.state = {
    tracks: [track(1, analysed), track(2), track(5, analysed)],
    playlists: [playlist(10, 'Favorites 1', [1]), playlist(11, 'Set', [2])],
  };
  hooks.added = [track(6), track(7, { tempo: 12000, key: '4A' }), track(8)];
  await initLibrary();
});

describe('landing imported tracks', () => {
  it('shows the same rows as a full reload, without one', async () => {
    const before = loadStateCalls();
    await importFiles(['/new'], { targetSource: 'Favorites 1' });
    expect(loadStateCalls()).toBe(before);
    const landed = snapshot();
    expect(landed.map((t: { id: number }) => t.id)).toEqual([1, 2, 5, 6, 7, 8]);

    hooks.state = {
      tracks: [...hooks.state.tracks, ...hooks.added],
      playlists: [playlist(10, 'Favorites 1', [1, 6, 7, 8]), playlist(11, 'Set', [2])],
    };
    await loadState();
    expect(landed).toEqual(snapshot());
  });

  it('lands dropped files the same way', async () => {
    const result = await importDroppedTracks(['/music']);
    const landed = snapshot();
    expect(result?.ids.sort()).toEqual([1, 2, 5, 6, 7, 8]);

    hooks.state = { ...hooks.state, tracks: [...hooks.state.tracks, ...hooks.added] };
    await loadState();
    expect(landed).toEqual(snapshot());
  });
});
