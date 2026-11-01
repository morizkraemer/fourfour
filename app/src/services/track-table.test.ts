import { describe, expect, it } from 'vitest';
import { decodeTrackTable, isTrackTable, type TrackTable } from './track-table.ts';

const WIRE =
  '{"strings":["Artist","","House","8A","#ff0000"],"folder_parent":[0,1,2,0],"folder_name":["/","m/","Artist/",""],' +
  '"id":[7,9],"dir":[2,3],"file":["one.mp3","two.wav"],"title":["One",""],"artist":[0,0],"album":[1,1],"genre":[2,1],"key":[3,1],' +
  '"tempo":[12800,0],"duration_secs":[200.5,0.0],"bitrate":[320,0],"file_size":[1234,5],"flags":[5,12],"ratings":[0,4],"colors":[1,4]}';

describe('track table', () => {
  it('decodes the load_state wire table into the same track objects the old list carried', () => {
    const table = JSON.parse(WIRE) as TrackTable;
    expect(isTrackTable(table)).toBe(true);
    expect(isTrackTable([])).toBe(false);
    const tracks = decodeTrackTable(table);
    expect(tracks).toStrictEqual([
      {
        id: 7,
        source_path: '/m/Artist/one.mp3',
        title: 'One',
        artist: 'Artist',
        album: '',
        genre: 'House',
        tempo: 12800,
        key: '8A',
        duration_secs: 200.5,
        bitrate: 320,
        file_size: 1234,
        has_artwork: true,
        has_cues: false,
        has_analysis: true,
        analysis_pending: false,
        rating: 4,
      },
      {
        id: 9,
        source_path: 'two.wav',
        title: '',
        artist: 'Artist',
        album: '',
        genre: '',
        tempo: 0,
        key: '',
        duration_secs: 0,
        bitrate: 0,
        file_size: 5,
        has_artwork: false,
        has_cues: false,
        has_analysis: true,
        analysis_pending: true,
        color: '#ff0000',
      },
    ]);
    expect(Object.keys(tracks[0])).toEqual([
      'id', 'source_path', 'title', 'artist', 'album', 'genre', 'tempo', 'key', 'duration_secs', 'bitrate', 'file_size',
      'has_artwork', 'has_cues', 'has_analysis', 'analysis_pending', 'rating',
    ]);
  });
});
