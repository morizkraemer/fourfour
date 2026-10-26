import { TRACKS, genBands } from './data.js';

export const T = TRACKS[0];

export const HOT = [
  { key: 'h1', slot: 1, letter: 'A', color: '#1fd35a', time: '0:16', timeMs: 16000, name: 'Intro', label: 'A Intro 0:16' },
  { key: 'h2', slot: 2, letter: 'B', color: '#33a1ff', time: '2:08', timeMs: 128000, name: 'Build', label: 'B Build 2:08' },
  { key: 'h3', slot: 3, letter: 'C', color: '#ffd600', time: '3:52', timeMs: 232000, name: 'Break', label: 'C Break 3:52' },
  { key: 'h4', slot: 4, letter: 'D', color: '#ff8a00', time: '4:25', timeMs: 265000, name: 'Drop', label: 'D Drop 4:25' },
];

export const MEMORY = [
  { key: 'm430000', slot: 0, letter: 'M', color: '#ff3b30', time: '7:10', timeMs: 430000, name: '', label: 'M 7:10' },
];

export const ALL_CUES = [...HOT, ...MEMORY];

export const CUE_PALETTE = [
  { name: 'Pink', color: '#ff2fa3' },
  { name: 'Red', color: '#ff3b30' },
  { name: 'Orange', color: '#ff8a00' },
  { name: 'Yellow', color: '#ffd600' },
  { name: 'Lime', color: '#a8e200' },
  { name: 'Green', color: '#1fd35a' },
  { name: 'Mint', color: '#17d9a3' },
  { name: 'Aqua', color: '#1fd0e8' },
  { name: 'Sky', color: '#33a1ff' },
  { name: 'Blue', color: '#3b5bff' },
  { name: 'Indigo', color: '#6a4cff' },
  { name: 'Purple', color: '#a64cff' },
  { name: 'Magenta', color: '#d83cff' },
  { name: 'Rose', color: '#ff5c8a' },
  { name: 'Peach', color: '#ffa07a' },
  { name: 'White', color: '#e6e6e6' },
];

export const BANDS_STRIP = genBands(11, 400);
export const BANDS_WIDE = genBands(11, 700);
export const BANDS_ZOOM = genBands(29, 120);

export const FILE_PATH = '/Users/merlin/Music/Kiln/Afterimage EP/01 Afterimage (Extended Mix).flac';

export const SEL_COLOR_INDEX = 2;
