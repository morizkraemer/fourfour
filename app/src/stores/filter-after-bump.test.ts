import { describe, expect, it, vi } from 'vitest';

const loaded = vi.hoisted(() => ({ tracks: [] as Record<string, unknown>[] }));

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
  invoke: vi.fn(async (command: string) => (command === 'load_state' ? { tracks: loaded.tracks, playlists: [] } : undefined)),
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

import { bumpLibraryTracks, filterTracks, library, loadState } from './library.svelte.ts';

const TITLES = ['house party', 'deep house', 'dub plate', 'amor mio', 'hot summer', 'a b c', 'bab', 'zzz 1', 'x2 y3', 'Éclair'];

function info(id: number, title: string) {
  return {
    id,
    source_path: `/m/${id}.mp3`,
    title,
    artist: '',
    album: '',
    genre: '',
    tempo: 12000,
    key: '',
    duration_secs: 0,
    bitrate: 320,
    file_size: 1,
    has_artwork: false,
    has_cues: false,
    has_analysis: true,
    analysis_pending: false,
  };
}

type Row = { id: number; title: string; artist: string; album: string; label: string; genre: string; key: string; bpm: string; time: string; filePath: string; index: string; raw: Record<string, unknown> };

const matching = (query: string) => {
  const terms = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  return (library.tracks as Row[])
    .filter((row) => {
      const fields = [row.title, row.artist, row.album, row.label, row.genre, row.key, row.bpm, row.time, row.filePath, row.index, row.raw.title, row.raw.artist, row.raw.album, row.raw.genre, row.raw.key, row.raw.source_path];
      return terms.every((term) => fields.some((field) => String(field ?? '').toLowerCase().includes(term)));
    })
    .map((row) => row.id);
};

const ids = (query: string) => filterTracks(library.tracks, query).map((row) => (row as { id: number }).id);

describe('a query typed after bumps starts from the rows of the earlier queries', () => {
  it('finds rows that changed into and out of the earlier result', async () => {
    loaded.tracks = Array.from({ length: 60 }, (_, i) => info(i + 1, TITLES[i % TITLES.length]));
    await loadState();
    const rows = () => library.tracks as { id: number; title: string }[];
    const queries = ['h', 'ho', 'hou', 'house', 'a', 'a b', 'b', 'ab', 'am', 'amo', 'x2 y3', '1 2', 'h o', 'zzz'];
    const edits = [
      [0, 'plain'],
      [3, 'house of pain'],
      [7, 'amor house'],
      [9, 'a b house'],
      [10, 'Éclair 1 2'],
      [20, ''],
    ] as const;
    for (const query of queries) expect(ids(query)).toEqual(matching(query));
    let step = 0;
    for (const [at, title] of edits) {
      rows()[at].title = title;
      bumpLibraryTracks([rows()[at].id]);
      for (const query of [queries[step % queries.length], queries[(step * 5 + 3) % queries.length], 'house', 'ho', 'a b', 'house o', 'amor h', 'a b h', 'hou p', 'ho e']) {
        expect(ids(query), `query "${query}" after editing row ${at}`).toEqual(matching(query));
      }
      step++;
    }
    for (const query of queries) expect(ids(query), `query "${query}" at the end`).toEqual(matching(query));
  });

  it('matches several single-character terms in any order', async () => {
    loaded.tracks = Array.from({ length: 40 }, (_, i) => info(i + 1, TITLES[i % TITLES.length]));
    await loadState();
    for (const query of ['a b', 'b a', 'a a', 'a 1', '1 a', 'x 2', 'a b c', 'é a']) expect(ids(query), query).toEqual(matching(query));
  });
});
