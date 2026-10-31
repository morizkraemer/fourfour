import type { LoadedState, PlaylistInput, TrackInfo } from '../src/services/tauri.svelte.ts';
import { mockAnalysisData } from '../src/mocks/analysis.ts';

const ARTISTS = ['Solomun', 'Tale Of Us', 'Bicep', 'Floating Points', 'Jamie xx', 'Nina Kraviz', 'Four Tet', 'Ben Böhmer'];
const GENRES = ['Melodic House', 'Melodic Techno', 'Breakbeat', 'Deep House'];
const KEYS = ['1A', '4B', '5A', '7A', '8B', '9A', '10B', '12A'];

const PERF_TRACKS = Number(process.env.FF_PERF_TRACKS ?? 0);

export const TRACK_COUNT = PERF_TRACKS || 20;

function fixtureTrack(i: number): TrackInfo {
  const n = i + 1;
  const title = `Smoke Track ${String(n).padStart(2, '0')}`;
  return {
    id: n,
    source_path: `/Music/smoke/${title}.mp3`,
    title,
    artist: ARTISTS[i % ARTISTS.length],
    album: `Smoke Album ${(i % 5) + 1}`,
    genre: GENRES[i % GENRES.length],
    tempo: (120 + ((i * 3) % 24)) * 100,
    key: KEYS[i % KEYS.length],
    duration_secs: 180 + i * 7,
    bitrate: 320,
    file_size: 9_000_000 + i * 100_000,
    has_artwork: false,
    has_cues: true,
    has_analysis: true,
    analysis_pending: false,
  };
}

const PERF_ROW_BANDS = !!process.env.FF_PERF_ROW_BANDS;

function rowBands(i: number) {
  const bytes = new Uint8Array(128 * 3);
  for (let c = 0; c < 128; c++) {
    bytes[c] = 40 + ((c * 7 + i) % 80);
    bytes[128 + c] = 20 + ((c * 3 + i) % 60);
    bytes[256 + c] = 10 + ((c * 5 + i) % 40);
  }
  return Buffer.from(bytes).toString('base64');
}

export function fixtureTracks(): TrackInfo[] {
  return Array.from({ length: TRACK_COUNT }, (_, i) => (PERF_ROW_BANDS ? { ...fixtureTrack(i), waveform_bands: rowBands(i) } : fixtureTrack(i)));
}

export function fixturePlaylists(): PlaylistInput[] {
  const playlists: PlaylistInput[] = [
    { id: 1, name: 'Peak Time', track_ids: [1, 2, 3], parent_id: 0, is_folder: false },
    { id: 2, name: 'Warmup', track_ids: [4], parent_id: 0, is_folder: false },
  ];
  if (PERF_TRACKS) {
    const big = Array.from({ length: Math.min(2000, PERF_TRACKS) }, (_, i) => i * Math.floor(PERF_TRACKS / Math.min(2000, PERF_TRACKS)) + 1);
    playlists.push({ id: 3, name: 'Big Playlist', track_ids: big, parent_id: 0, is_folder: false });
  }
  return playlists;
}

export function fixtureState(): LoadedState {
  return { tracks: fixtureTracks(), playlists: fixturePlaylists() };
}

/** The fixture with `count` tracks, the first ones as in `fixtureState`, for behaviour that depends on library size. */
export function largeFixtureState(count: number): LoadedState {
  return { tracks: Array.from({ length: count }, (_, i) => fixtureTrack(i)), playlists: fixturePlaylists() };
}

export function fixtureAnalysis(trackId: number) {
  const track = trackId >= 1 && trackId <= TRACK_COUNT ? fixtureTrack(trackId - 1) : undefined;
  if (!track) throw new Error(`fixture has no track ${trackId}`);
  return mockAnalysisData(track.id, track.duration_secs, track.tempo, track.key);
}
