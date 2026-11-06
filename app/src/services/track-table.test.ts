import { describe, expect, it } from 'vitest';
import { fixtureBuffer, fixtureText } from './fixtures.ts';
import { decodeStateBytes, decodeTrackTable, isTrackTable, type TrackTable } from './track-table.ts';

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

describe('binary load_state answer', () => {
  const fixture = fixtureBuffer;
  const jsonState = (name: string) => JSON.parse(fixtureText(name));

  for (const name of ['state-edge', 'state-empty']) {
    it(`${name}: decodes to the objects the JSON table gives, field by field and key by key`, () => {
      const json = jsonState(`${name}.json`);
      const expected = decodeTrackTable(json.tracks);
      const { tracks, playlists } = decodeStateBytes(fixture(`${name}.bin`));
      expect(tracks).toStrictEqual(expected);
      expect(tracks.map((t) => Object.keys(t))).toEqual(expected.map((t) => Object.keys(t)));
      expect(playlists).toStrictEqual(json.playlists);
    });
  }

  it('keeps empty strings, quotes, backslashes, non-ASCII, a leading BOM and big sizes', () => {
    const { tracks, playlists } = decodeStateBytes(fixture('state-edge.bin'));
    expect(tracks[0].title).toBe('');
    expect(tracks[1].title).toBe('He said "hi" \\ bye');
    expect(tracks[1].source_path).toBe('C:\\Music\\Ünï\\song "x".flac');
    expect(tracks[1].genre).toBe('\u{1f600} emoji');
    expect(tracks[1].file_size).toBe(2 ** 53);
    expect(tracks[2].title).toBe('\ufeffBOM title');
    expect(tracks[2].source_path).toBe('relative.mp3');
    expect(tracks[2]).toMatchObject({ rating: 0, color: '' });
    expect(tracks[3].source_path).toBe('/m/日本/曲.mp3');
    expect(tracks[3].title).toBe('a\u0000b\n\t');
    expect(tracks[4]).toMatchObject({ id: 4294967295, tempo: 0, file_size: 18446744073709552000, duration_secs: 1e21 });
    expect(playlists[0].name).toBe('Set "A" \u{1f3a7}');
  });

  it('refuses a buffer that is not a track table', () => {
    expect(() => decodeStateBytes(new ArrayBuffer(64))).toThrow('unknown buffer');
  });
});
