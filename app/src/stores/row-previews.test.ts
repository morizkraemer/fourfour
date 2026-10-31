import { beforeEach, describe, expect, it, vi } from 'vitest';

const hooks = vi.hoisted(() => ({
  calls: [] as number[][],
  fail: false,
}));

function base64(bytes: Uint8Array) {
  let text = '';
  for (const b of bytes) text += String.fromCharCode(b);
  return btoa(text);
}

const BANDS = base64(new Uint8Array(384).map((_, i) => 20 + (i % 90)));
const MONO = base64(new Uint8Array(400).map((_, i) => 1 + (i % 30)));

function info(id: number, analysed = true) {
  return {
    id,
    source_path: `/m/${id}.mp3`,
    title: `T${id}`,
    artist: 'A',
    album: 'B',
    genre: 'House',
    tempo: analysed ? 12400 : 0,
    key: analysed ? '8A' : '',
    duration_secs: 200,
    bitrate: 320,
    file_size: 1,
    has_artwork: false,
    has_cues: false,
    has_analysis: analysed,
    analysis_pending: false,
  };
}

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
  invoke: vi.fn(async (command: string, args: Record<string, any> = {}) => {
    if (command === 'load_state') return { tracks: [info(1), info(2), info(3), info(4, false)], playlists: [] };
    if (command === 'get_row_previews') {
      hooks.calls.push([...args.trackIds]);
      if (hooks.fail) throw new Error('backend gone');
      const all = [
        { id: 1, waveform_bands: BANDS },
        { id: 2, waveform_preview: MONO },
      ];
      return all.filter((a) => args.trackIds.includes(a.id));
    }
    return undefined;
  }),
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

import { library, loadState, requestRowPreviews } from './library.svelte.ts';

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

beforeEach(async () => {
  hooks.calls = [];
  hooks.fail = false;
  await loadState();
});

describe('row previews on demand', () => {
  it('loads the list without previews and fetches the rows on screen in one call', async () => {
    const tracks = library.tracks;
    expect(tracks.every((t) => t.bands === null)).toBe(true);
    requestRowPreviews(tracks.slice(0, 2));
    requestRowPreviews(tracks);
    await settle();
    expect(hooks.calls).toEqual([[1, 2, 3]]);
    expect(tracks[0].bands).not.toBeNull();
    expect(tracks[1].bands).not.toBeNull();
    expect(tracks[2].bands).toBeNull();
    expect(tracks[3].bands).toBeNull();
  });

  it('asks once per track object and again after a reload', async () => {
    requestRowPreviews(library.tracks);
    await settle();
    requestRowPreviews(library.tracks);
    await settle();
    expect(hooks.calls).toHaveLength(1);
    await loadState();
    requestRowPreviews(library.tracks);
    await settle();
    expect(hooks.calls).toHaveLength(2);
  });

  it('asks again after a failed call', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    vi.stubGlobal('window', { __TAURI_INTERNALS__: {} });
    hooks.fail = true;
    requestRowPreviews(library.tracks);
    await settle();
    expect(warn).toHaveBeenCalled();
    hooks.fail = false;
    requestRowPreviews(library.tracks);
    await settle();
    expect(hooks.calls).toEqual([[1, 2, 3], [1, 2, 3]]);
    expect(library.tracks[0].bands).not.toBeNull();
    warn.mockRestore();
    vi.unstubAllGlobals();
  });
});
