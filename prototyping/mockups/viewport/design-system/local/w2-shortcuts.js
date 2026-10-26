const LETTERS = 'ABCDEFGH';

function hotCues(prefix, label) {
  return Array.from({ length: 8 }, (_, i) => ({
    id: `${label}${i + 1}`,
    label: label === 'set' ? `Hot cue ${LETTERS[i]}: set at playhead or jump` : `Delete hot cue ${LETTERS[i]}`,
    binding: label === 'set' ? `⇧${i + 1}` : `⌥⇧${i + 1}`,
    custom: false,
  }));
}

const FAVORITES = Array.from({ length: 9 }, (_, i) => ({
  id: `fav${i + 1}`,
  label: `Toggle favorite ${i + 1} on selection`,
  binding: String(i + 1),
  custom: false,
}));

export const SHORTCUT_GROUPS = [
  {
    group: 'General',
    actions: [
      { id: 'palette', label: 'Command palette', binding: '⌘K' },
      { id: 'settings', label: 'Open settings', binding: '⌘,' },
      { id: 'shortcuts', label: 'Show keyboard shortcuts', binding: null },
      { id: 'dismiss', label: 'Close settings, clear search, or collapse player', binding: 'Esc' },
      { id: 'next', label: 'Focus next region', binding: 'F6' },
      { id: 'prev', label: 'Focus previous region', binding: '⇧F6' },
    ],
  },
  { group: 'Playback', actions: [{ id: 'play', label: 'Play / pause', binding: 'Space' }] },
  {
    group: 'Cues',
    actions: [
      { id: 'cue', label: 'Cue: return to cue point, or set it at the playhead', binding: 'C' },
      { id: 'mem', label: 'Add memory cue at playhead', binding: 'M' },
      ...hotCues('h', 'set'),
      ...hotCues('d', 'del'),
    ],
  },
  {
    group: 'Library',
    actions: [
      { id: 'search', label: 'Search tracks', binding: '⌘F' },
      { id: 'newpl', label: 'New playlist', binding: '⌘⇧N', custom: true },
      { id: 'import', label: 'Import files…', binding: '⌘O' },
      { id: 'analyze', label: 'Analyze selection', binding: null },
      { id: 'sync', label: 'Sync stick', binding: null },
    ],
  },
  {
    group: 'View',
    actions: [
      { id: 'player', label: 'Toggle big / small player', binding: 'Tab' },
      { id: 'org', label: 'Toggle organize drawer', binding: '⇧Tab' },
      { id: 'insp', label: 'Toggle inspector', binding: null },
    ],
  },
  { group: 'Favorites', actions: FAVORITES },
];
