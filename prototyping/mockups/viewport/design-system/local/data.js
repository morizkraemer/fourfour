const CUE_COLORS = ['#ec4899', '#38bdf8', '#4ade80', '#f59e0b', '#a78bfa', '#facc15', '#94a3b8'];
export { CUE_COLORS };

function rng(seed) {
  let s = seed * 9301 + 49297;
  return () => {
    s = (s * 9301 + 49297) % 233280;
    return s / 233280;
  };
}

export function genBands(seed, n = 128) {
  const r = rng(seed);
  const low = [];
  const mid = [];
  const high = [];
  const sections = [0.08, 0.28, 0.4, 0.46, 0.7, 0.9, 1];
  const lows = [0.25, 0.7, 0.35, 0.1, 0.85, 0.6, 0.2];
  const shift = seed % 5;
  for (let i = 0; i < n; i++) {
    const t = i / n;
    const s = sections.findIndex((e) => t <= e);
    const base = lows[(s + shift) % lows.length];
    const pulse = 0.55 + 0.45 * Math.abs(Math.sin(i * 0.9));
    low.push(Math.min(1, base * pulse * (0.9 + r() * 0.2)));
    mid.push(Math.min(1, (0.15 + base * 0.45) * (0.6 + r() * 0.5)));
    high.push(Math.min(1, (0.1 + base * 0.35) * (0.4 + r() * 0.8)));
  }
  return { low, mid, high };
}

const PALETTES = [
  ['#0f2a43', '#1d6fa5', '#f2c14e'],
  ['#2b0f2e', '#c2185b', '#ffb199'],
  ['#0d1f17', '#2e7d5b', '#d9f99d'],
  ['#1a1a2e', '#6a4c93', '#f4a261'],
  ['#2a1a0a', '#b5651d', '#fde68a'],
  ['#101820', '#e63946', '#f1faee'],
  ['#0b132b', '#3a86ff', '#ffd6a5'],
  ['#1b1b1b', '#9aa0a6', '#f5f5f5'],
];

export function cover(seed, size = 120) {
  const [a, b, c] = PALETTES[seed % PALETTES.length];
  const r = rng(seed + 3);
  const cx = 30 + r() * 60;
  const cy = 30 + r() * 60;
  const svg = `<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 120 120' width='${size}' height='${size}'><defs><linearGradient id='g' x1='0' y1='0' x2='1' y2='1'><stop offset='0' stop-color='${a}'/><stop offset='1' stop-color='${b}'/></linearGradient></defs><rect width='120' height='120' fill='url(#g)'/><circle cx='${cx}' cy='${cy}' r='${24 + r() * 22}' fill='${c}' opacity='.85'/><circle cx='${120 - cx}' cy='${120 - cy}' r='${10 + r() * 14}' fill='${a}' opacity='.8'/></svg>`;
  const encoded = encodeURIComponent(svg).replace(/\(/g, '%28').replace(/\)/g, '%29').replace(/'/g, '%27');
  return `data:image/svg+xml;utf8,${encoded}`;
}

const RAW = [
  ['Afterimage (Extended Mix)', 'Mira Sol', 'Afterimage EP', 'Kiln', '125.00', '8A', '8:11', 'Melodic Techno', 2025, 4, '#f59e0b', 'Opener for Peak Time. Break at 3:52.'],
  ['Silent Hours', 'Mira Sol', 'Nocturnes', 'Nightform', '124.00', '8A', '6:42', 'Deep House', 2024, 5, null, ''],
  ['Glass Harbor', 'Ode & Ash', 'Glass Harbor', 'Kiln', '126.00', '9A', '7:05', 'Melodic Techno', 2025, 3, null, ''],
  ['Low Tide Ritual', 'Tomas Reyne', 'Salt', 'Nightform', '122.00', '5A', '5:58', 'Deep House', 2023, 4, '#4ade80', 'Warm up, vocal sample at 2:10'],
  ['Copper Lines (Hale Rework)', 'Hale', 'Copper Lines', 'Southbound', '128.00', '10B', '6:20', 'Tech House', 2025, 3, null, ''],
  ['Paper Moons', 'Ivo Brandt', 'Paper Moons', 'Southbound', '123.00', '6A', '6:03', 'Deep House', 2022, 2, null, ''],
  ['Harbor Light', 'Ode & Ash', 'Glass Harbor', 'Nightform', '127.00', '11A', '7:34', 'Melodic Techno', 2025, 5, '#ec4899', 'Peak. Pair with Afterimage'],
  ['Slow Orbit', 'Tomas Reyne', 'Salt', 'Kiln', '121.00', '4A', '5:47', 'Ambient', 2023, 3, null, ''],
  ['Neon Parade', 'Lena Vogt', 'Parade', 'Mainstage Rec.', '130.00', '12A', '6:55', 'Tech House', 2025, 4, '#38bdf8', ''],
  ['Fault Lines', 'Danya', 'Self Destructive Mechanisms', 'Regent', '140.00', '1A', '6:03', 'Techno', 2024, 4, null, ''],
  ['Context 004 (Franco Rossi Remix)', 'Eme Kulhnek', 'Context 004', 'Concept', '144.00', '2A', '5:50', 'Techno', 2021, 3, null, ''],
  ['Efficient', 'Regent', 'Simulation', 'Regent', '142.00', '1A', '5:55', 'Techno', 2024, 4, null, ''],
  ['Mercamadrid', 'No.name', 'BTVA002', 'BTVA', '143.00', '9A', '5:13', 'Techno', 2023, 2, null, ''],
  ['Midnight Freight', 'Hale', 'Southbound 07', 'Southbound', '126.00', '7A', '7:21', 'Tech House', 2025, 3, null, ''],
  ['Quiet Machines', 'Ivo Brandt', 'Quiet Machines', 'Kiln', '122.00', '3A', '6:48', 'Deep House', 2024, 4, '#a78bfa', ''],
  ['Static Bloom', 'Lena Vogt', 'Parade', 'Mainstage Rec.', '129.00', '12B', '7:02', 'Progressive', 2025, 3, null, ''],
  ['Ember Room', 'Mira Sol', 'Nocturnes', 'Nightform', '124.00', '8B', '6:30', 'Deep House', 2024, 4, null, ''],
  ['Tidal Wire', 'Ode & Ash', 'Wire', 'Kiln', '125.00', '9A', '7:44', 'Melodic Techno', 2026, 5, null, ''],
  ['Subway Hymn', 'Tomas Reyne', 'Salt', 'Nightform', '123.00', '4B', '5:35', 'Deep House', 2023, 3, null, ''],
  ['Kiln Session 3', 'Various', 'Kiln Sessions', 'Kiln', '127.00', '10A', '8:20', 'Melodic Techno', 2022, 2, null, ''],
  ['Nine Floors Down', 'Danya', 'Self Destructive Mechanisms', 'Regent', '138.00', '3A', '6:41', 'Techno', 2024, 4, null, ''],
  ['Late Arrival', 'Hale', 'Southbound 07', 'Southbound', '126.00', '6B', '6:09', 'Tech House', 2025, 3, null, ''],
];

export const CUES = [
  { id: 'A', name: 'Intro', position: '0:16', color: CUE_COLORS[0], frac: 0.03 },
  { id: 'B', name: 'Build', position: '2:08', color: CUE_COLORS[1], frac: 0.26 },
  { id: 'C', name: 'Break', position: '3:52', color: CUE_COLORS[2], frac: 0.47 },
  { id: 'D', name: 'Drop', position: '4:25', color: CUE_COLORS[3], frac: 0.54 },
  { id: 'E', name: 'Outro', position: '7:10', color: CUE_COLORS[4], frac: 0.88 },
];

export const TRACKS = RAW.map((r, i) => ({
  id: i + 1,
  index: i + 1,
  title: r[0],
  artist: r[1],
  album: r[2],
  label: r[3],
  bpm: r[4],
  key: r[5],
  time: r[6],
  genre: r[7],
  year: r[8],
  rating: r[9],
  color: r[10],
  tag: r[10] ? { type: 'color', color: r[10] } : null,
  comment: r[11],
  fav: i === 0 || i === 6 ? (i === 0 ? 1 : 2) : null,
  bands: genBands(i + 11, 128),
  rowCover: cover(i + 1, 40),
  cover: cover(i + 1, 300),
}));

export const SELECTED = TRACKS[0];
export const DETAIL_BANDS = genBands(11, 400);
export const PLAYER_BANDS = genBands(11, 900);

export const LIBRARY_COUNT = '30,412';

export const SIDEBAR = {
  favorites: [
    { kind: 'favorite', digit: 1, label: 'Favorites 1', count: 124 },
    { kind: 'favorite', digit: 2, label: 'Favorites 2', count: 38 },
  ],
  library: [
    { kind: 'leaf', icon: 'list', label: 'All Tracks', count: '30,412' },
    { kind: 'leaf', icon: 'clock-3', label: 'Recently Added', count: 217 },
  ],
  playlists: [
    { kind: 'playlist', label: 'Peak Time', count: 148 },
    { kind: 'playlist', label: 'Warm Up', count: 96 },
    { kind: 'playlist', label: 'After Hours', count: 72 },
    { kind: 'playlist', label: 'Saturday Closing', count: 41 },
    { kind: 'playlist', label: 'Rooftop 2026', count: 33 },
    { kind: 'playlist', label: 'Bar Sets', count: 210 },
    { kind: 'playlist', label: 'To Check', count: 17 },
  ],
  usb: [
    { kind: 'usb', label: 'SANDISK_64', status: 'online' },
    { kind: 'usb', label: 'TOURSTICK', status: 'offline' },
  ],
};

export const PLAYLIST_ROWS = TRACKS.slice(3, 15);
