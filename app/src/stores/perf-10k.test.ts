import { describe, it } from 'vitest';
import { clearSort, setSort, sortTracks } from './table-columns.svelte.ts';
import { filterTracks, library, tracksForSidebarSource } from './library.svelte.ts';

interface CpuUsage {
  user: number;
  system: number;
}
const node = (globalThis as unknown as { process: { env: Record<string, string | undefined>; cpuUsage(previous?: CpuUsage): CpuUsage } }).process;

const TRACKS = Number(node.env.FF_PERF_TRACKS ?? 0);
const REPEATS = 7;

const ARTISTS = ['Solomun', 'Tale Of Us', 'Bicep', 'Floating Points', 'Jamie xx', 'Nina Kraviz', 'Four Tet', 'Ben Böhmer'];

function localTrack(i: number) {
  return {
    id: i + 1,
    title: `Smoke Track ${i + 1}`,
    artist: ARTISTS[i % ARTISTS.length],
    album: `Smoke Album ${i % 5}`,
    genre: 'House',
    key: `${(i % 12) + 1}A`,
    bpm: String(120 + (i % 24)),
    time: `${3 + (i % 5)}:${String(i % 60).padStart(2, '0')}`,
    filePath: `/Music/smoke/Smoke Track ${i + 1}.mp3`,
    index: String(i + 1),
    raw: { title: `Smoke Track ${i + 1}`, artist: ARTISTS[i % ARTISTS.length], source_path: `/Music/smoke/Smoke Track ${i + 1}.mp3`, tempo: 12000 },
  };
}

function cpuMs(run: () => void): number {
  const before = node.cpuUsage();
  run();
  const after = node.cpuUsage(before);
  return (after.user + after.system) / 1000;
}

const median = (values: number[]) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];

function measure(name: string, run: () => void, reset?: () => void) {
  const samples: number[] = [];
  for (let n = 0; n < REPEATS; n++) {
    reset?.();
    samples.push(cpuMs(run));
  }
  console.log(`${name.padEnd(46)} median ${median(samples).toFixed(1).padStart(7)} ms   first ${samples[0].toFixed(1).padStart(7)} ms`);
}

describe.skipIf(TRACKS === 0)('store hot paths at scale (cpu ms)', () => {
  it('measures', () => {
    library.tracks = Array.from({ length: TRACKS }, (_, i) => localTrack(i));
    library.playlists = [{ id: 1, name: 'Big', track_ids: Array.from({ length: Math.min(2000, TRACKS) }, (_, i) => i * Math.floor(TRACKS / Math.min(2000, TRACKS)) + 1), parent_id: 0, is_folder: false }];

    measure('open 2k playlist (tracksForSidebarSource)', () => tracksForSidebarSource('Big'));

    const swapSource = () => {
      library.tracks = [...library.tracks];
      filterTracks(library.tracks, 'qqqqq');
    };
    const typing = ['s', 'sm', 'smo', 'smok', 'smoke', 'smoke t', 'smoke tr', 'smoke tra', 'smoke trac', 'smoke track', 'smoke track 1', 'smoke track 12'];
    measure('search typing, 12 keystrokes, source array reused', () => {
      for (const q of typing) filterTracks(library.tracks, q);
    }, swapSource);
    measure('search typing, selective query (track 12)', () => {
      for (const q of ['t', 'tr', 'tra', 'trac', 'track', 'track 1', 'track 12']) filterTracks(library.tracks, q);
    }, swapSource);

    for (const key of ['title', 'artist', 'bpm', 'key']) {
      clearSort('library');
      setSort('library', key);
      measure(`sort by ${key}`, () => sortTracks(library.tracks, 'library'));
    }
    clearSort('library');
  });
});
