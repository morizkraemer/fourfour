export const MOCK_TRACKS = [
  { index: 1, title: 'Silent Hours', artist: 'Mira Sol', label: 'Nightform', bpm: '124.00', key: '8A', time: '6:42', fav: 1 },
  { index: 2, title: 'Glass Harbor', artist: 'Ode & Ash', label: 'Kiln', bpm: '126.00', key: '9A', time: '7:05' },
  { index: 3, title: 'Low Tide Ritual', artist: 'Tomas Reyne', label: 'Nightform', bpm: '122.00', key: '5A', time: '5:58', tag: { type: 'color', color: '#4ade80' } },
  { index: 4, title: 'Copper Lines', artist: 'Hale', label: 'Southbound', bpm: '128.00', key: '10B', time: '6:20' },
  { index: 5, title: 'Afterimage', artist: 'Mira Sol', label: 'Kiln', bpm: '125.00', key: '8A', time: '8:11', tag: { type: 'color', color: '#f59e0b' } },
  { index: 6, title: 'Paper Moons', artist: 'Ivo Brandt', label: 'Southbound', bpm: '123.00', key: '6A', time: '6:03' },
  { index: 7, title: 'Harbor Light', artist: 'Ode & Ash', label: 'Nightform', bpm: '127.00', key: '11A', time: '7:34' },
  { index: 8, title: 'Slow Orbit', artist: 'Tomas Reyne', label: 'Kiln', bpm: '121.00', key: '4A', time: '5:47' },
];

export const MOCK_SIDEBAR = {
  favorites: [
    { kind: 'favorite', digit: 1, label: 'Favorites 1', count: 12 },
    { kind: 'favorite', digit: 2, label: 'Favorites 2', count: 4 },
  ],
  library: [
    { kind: 'leaf', icon: 'list', label: 'All Tracks', count: 842 },
    { kind: 'leaf', icon: 'clock-3', label: 'Recently Added', count: 37 },
  ],
  playlists: [
    { kind: 'playlist', label: 'Peak Time', count: 48 },
    { kind: 'playlist', label: 'Warm Up', count: 31 },
    { kind: 'playlist', label: 'After Hours', count: 22 },
  ],
  usb: [{ kind: 'usb', label: 'SANDISK_16', status: 'online' }],
};
