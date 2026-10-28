import type { LoadedState, PlaylistInput, TrackInfo } from '../src/services/tauri.svelte.ts';
import { mockAnalysisData } from '../src/mocks/analysis.ts';

const ARTISTS = ['Solomun', 'Tale Of Us', 'Bicep', 'Floating Points', 'Jamie xx', 'Nina Kraviz', 'Four Tet', 'Ben Böhmer'];
const GENRES = ['Melodic House', 'Melodic Techno', 'Breakbeat', 'Deep House'];
const KEYS = ['1A', '4B', '5A', '7A', '8B', '9A', '10B', '12A'];

export const TRACK_COUNT = 20;

export function fixtureTracks(): TrackInfo[] {
  return Array.from({ length: TRACK_COUNT }, (_, i) => {
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
  });
}

export function fixturePlaylists(): PlaylistInput[] {
  return [
    { id: 1, name: 'Peak Time', track_ids: [1, 2, 3], parent_id: 0, is_folder: false },
    { id: 2, name: 'Warmup', track_ids: [4], parent_id: 0, is_folder: false },
  ];
}

export function fixtureState(): LoadedState {
  return { tracks: fixtureTracks(), playlists: fixturePlaylists() };
}

export function fixtureAnalysis(trackId: number) {
  const track = fixtureTracks().find((t) => t.id === trackId);
  if (!track) throw new Error(`fixture has no track ${trackId}`);
  return mockAnalysisData(track.id, track.duration_secs, track.tempo, track.key);
}
