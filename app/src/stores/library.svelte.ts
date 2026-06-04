/**
 * library.svelte.ts — Demo library data for the Browse screen.
 * Tracks, columns, and sidebar structure lifted from the static artboard modules.
 * Will be replaced by real Tauri IPC data in Phase 6.
 */

/* ── Table column set (matches module-table.artboard.js) ────────────── */

export const TABLE_COLUMNS = [
  { key: 'fav', label: 'FAV', width: 20 },
  { key: 'index', label: '#', width: 24 },
  { key: 'wave', label: 'PREVIEW', width: 120 },
  { key: 'cover', label: '', width: 18 },
  { key: 'title', label: 'TITLE', flex: true },
  { key: 'artist', label: 'ARTIST', width: 180 },
  { key: 'label', label: 'LABEL', width: 130 },
  { key: 'bpm', label: 'BPM', width: 50, sort: 'desc' },
  { key: 'key', label: 'KEY', width: 36 },
  { key: 'time', label: 'TIME', width: 48 },
];

/* ── Demo tracks (from module-table.artboard.js) ────────────────────── */

export const tracks = [
  { index: '01', title: 'Hyperion',         artist: 'Solomun',                label: 'Diynamic',        bpm: '124', key: '9A',  time: '7:42', genre: 'Melodic House',     year: 2023 },
  { index: '02', title: 'Voidwalker',       artist: 'Tale Of Us',             label: 'Afterlife',       bpm: '122', key: '7A',  time: '8:14', fav: 1, genre: 'Melodic Techno', year: 2022,
    cues: [{ name: 'Intro', position: '0:00' }, { name: 'Drop', position: '1:32' }, { name: 'Break', position: '4:48' }],
    filePath: '/Volumes/Music/Tale Of Us/Endless Run/02 Voidwalker.flac',
    comment: 'Emotional peak.' },
  { index: '03', title: 'Drift',            artist: 'Bicep',                  label: 'Ninja Tune',      bpm: '128', key: '5B',  time: '6:55', genre: 'Breakbeat',          year: 2023 },
  { index: '04', title: 'Aphelion',         artist: 'Maceo Plex',             label: 'Lone Romantic',   bpm: '121', key: '6A',  time: '7:08', genre: 'Deep Techno',        year: 2021 },
  { index: '05', title: 'Resonate',         artist: 'Mind Against',           label: 'Life and Death',  bpm: '120', key: '8A',  time: '7:34', genre: 'Melodic Techno',     year: 2022 },
  { index: '06', title: 'Glass Ladder',     artist: 'Adam Port',              label: 'Keinemusik',      bpm: '118', key: '4A',  time: '6:32', fav: 3, genre: 'Deep House', year: 2023 },
  { index: '07', title: 'Suspended',        artist: 'Stephan Bodzin',         label: 'Herzblut',        bpm: '124', key: '11A', time: '8:02', genre: 'Melodic Techno',     year: 2020 },
  { index: '08', title: 'Fractured Light',  artist: 'Recondite',              label: 'Plangent',        bpm: '121', key: '12B', time: '7:18', genre: 'Ambient Techno',     year: 2019 },
  { index: '09', title: 'Distant Star',     artist: 'Kiasmos',                label: 'Erased Tapes',    bpm: '119', key: '9B',  time: '6:48', genre: 'Electronica',        year: 2021 },
  { index: '10', title: 'Halflight',        artist: 'Kollektiv Turmstrasse',  label: 'Diynamic',        bpm: '122', key: '6A',  time: '7:55', genre: 'Deep House',         year: 2020 },
  { index: '11', title: 'Erosion',          artist: 'Innellea',               label: 'Afterlife',       bpm: '—',   key: '—',   time: '—',    genre: 'Melodic Techno',     year: 2024 },
  { index: '12', title: 'Cinder',           artist: 'Anfisa Letyago',         label: 'NTFO',            bpm: '130', key: '9A',  time: '6:14', fav: 2, genre: 'Techno',     year: 2023 },
];

export const trackCount = tracks.length;

/* ── Sidebar data ───────────────────────────────────────────────────── */

export const sidebarData = {
  favorites: {
    label: 'FAVORITES',
    rows: [
      { kind: 'favorite', digit: 1, label: 'Peak Time',  count: 24 },
      { kind: 'favorite', digit: 2, label: 'Warmup',     count: 18 },
    ],
  },
  library: {
    label: 'LIBRARY',
    rows: [
      { kind: 'leaf', icon: 'list',     label: 'All Tracks',      count: 1284 },
      { kind: 'leaf', icon: 'clock-3',  label: 'Recently Added',  count: 52 },
      { kind: 'leaf', icon: 'disc',     label: 'Unfiled',         count: 18 },
    ],
  },
  playlists: {
    label: 'PLAYLISTS',
    showAdd: true,
    rows: [
      { kind: 'playlist', label: 'Saturday',        count: 64 },
      { kind: 'playlist', label: 'Sunday Closing',  count: 40 },
      { kind: 'playlist', label: 'Festival Prep',   count: 28 },
    ],
  },
  usb: {
    label: 'USB DEVICES',
    rows: [
      { kind: 'usb', label: 'SanDisk 64GB', status: 'online' },
      { kind: 'usb', label: 'VAULT666',     status: 'offline' },
    ],
  },
};
