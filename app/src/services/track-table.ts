import type { TrackInfo } from './tauri.svelte.ts';

/**
 * The track list as `load_state` sends it: one array per field instead of one object per track. Each path is
 * split into its folder (`dir`, an index into the folder list) and its file name; a folder is its parent's path
 * plus its `folder_name`, and `folder_parent` is the parent's index plus one, 0 for none. Artists, albums,
 * genres, keys and colours are indexes into `strings`. Ratings and colours are `[row, value, ...]` pairs for
 * the tracks that have one.
 */
export interface TrackTable {
  strings: string[];
  folder_parent: number[];
  folder_name: string[];
  id: number[];
  dir: number[];
  file: string[];
  title: string[];
  artist: number[];
  album: number[];
  genre: number[];
  key: number[];
  tempo: number[];
  duration_secs: number[];
  bitrate: number[];
  file_size: number[];
  flags: number[];
  ratings: number[];
  colors: number[];
}

export const TRACK_FLAG = { artwork: 1, cues: 2, analysis: 4, pending: 8 } as const;

export function isTrackTable(tracks: TrackInfo[] | TrackTable): tracks is TrackTable {
  return !Array.isArray(tracks);
}

function folderPaths({ folder_parent, folder_name }: TrackTable): string[] {
  const paths: string[] = new Array(folder_name.length);
  for (let i = 0; i < folder_name.length; i++) {
    const parent = folder_parent[i];
    paths[i] = parent === 0 ? folder_name[i] : paths[parent - 1] + folder_name[i];
  }
  return paths;
}

export function decodeTrackTable(table: TrackTable): TrackInfo[] {
  const { strings, id, dir, file, title, artist, album, genre, key, tempo, duration_secs, bitrate, file_size, flags } = table;
  const folders = folderPaths(table);
  const tracks: TrackInfo[] = new Array(id.length);
  for (let i = 0; i < id.length; i++) {
    const f = flags[i];
    tracks[i] = {
      id: id[i],
      source_path: folders[dir[i]] + file[i],
      title: title[i],
      artist: strings[artist[i]],
      album: strings[album[i]],
      genre: strings[genre[i]],
      tempo: tempo[i],
      key: strings[key[i]],
      duration_secs: duration_secs[i],
      bitrate: bitrate[i],
      file_size: file_size[i],
      has_artwork: (f & TRACK_FLAG.artwork) !== 0,
      has_cues: (f & TRACK_FLAG.cues) !== 0,
      has_analysis: (f & TRACK_FLAG.analysis) !== 0,
      analysis_pending: (f & TRACK_FLAG.pending) !== 0,
    };
  }
  const { ratings, colors } = table;
  for (let i = 0; i < ratings.length; i += 2) tracks[ratings[i]].rating = ratings[i + 1];
  for (let i = 0; i < colors.length; i += 2) tracks[colors[i]].color = strings[colors[i + 1]];
  return tracks;
}
