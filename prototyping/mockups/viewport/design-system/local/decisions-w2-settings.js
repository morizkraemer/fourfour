export const DECISIONS = {
  title: 'Settings round 2: overhaul (F21)',
  feedback: [
    { id: 'F21', text: 'Settings needs a UI overhaul. Today: full-window takeover with General, Audio, Keyboard shortcuts, Debug. Inventory: library database path, Rekordbox import, version, engine, output device, sample rate, buffer, 42 shortcuts, library counts, IPC log, reset. Backend-only today: analyze new tracks on import (config.json, default on), analysis engine, worker count.' },
  ],
  options: [
    {
      tag: 'A',
      name: 'Sheet by object',
      idea: 'Centred sheet over the library. The list is by object (Library, Analysis, Audio, Keyboard, Advanced) and each entry shows its live state. Short pages: a status card, then compact rows.',
      board: 'settings-2/option-a, option-a-keys',
    },
    {
      tag: 'B',
      name: 'Settings in the palette',
      idea: 'Settings are rows in ⌘K with their control on the right (toggle, segment, dropdown). A keyboard map replaces the shortcut list. A small page keeps log and details.',
      board: 'settings-2/option-b, option-b-keys',
    },
    {
      tag: 'C',
      name: 'Side panel next to the player',
      idea: 'The right slot becomes a Settings panel (status-bar gear). Accordion with a summary per section; non-modal, so you hear audio changes with the library still visible.',
      board: 'settings-2/option-c',
    },
  ],
  matrix: [
    { q: 'Time to change one known setting', cells: ['Open, click page, change (3 steps)', '+⌘K, type, change (3 keys)', 'Gear, find section, change'] },
    { q: 'Seeing everything / first look', cells: ['+Five short pages with live state', '-Only what you search for', 'Accordion, one screen'] },
    { q: 'Hear a change while looking at the player', cells: ['-Modal, covers the app', '-Palette closes on Esc', '+Yes, non-modal, test tone and meter'] },
    { q: 'Room to explain a setting', cells: ['+One line each, status cards', '-Almost none', '-Labels only'] },
    { q: 'Shortcuts', cells: ['Filter plus 2 columns', '+Keyboard map: shows what is on each key', 'Opens the map or a sheet'] },
    { q: 'New analysis settings (import toggle, engine, workers)', cells: ['+Own page with live queue', 'Three rows found by "analy"', 'Section with summary'] },
    { q: 'New work', cells: ['Sheet, nav, status cards', '+Rows with controls in the palette; map is new', 'Panel in an existing slot'] },
  ],
  recommendation: {
    pick: 'A as the home, B as the fast path. A sheet by object for the first look and the explanations; every setting also shows up in ⌘K with its control, so a known setting costs three keys. The keyboard map (B) replaces the shortcut list later.',
    why: [
      'Settings are opened rarely. The first look should explain itself, which a short page with a live status card does and a palette cannot.',
      'The palette already exists and already lists shortcuts. Rows with a control cost little and make the daily ones instant.',
      'The map answers "what is on this key" and "what is free", which a 42-row list never will. It also folds hot cue A to H and favorites 1 to 9 into key rows.',
      'C\'s test tone and level meter are worth stealing for A\'s Audio page; the panel itself costs the inspector slot.',
    ],
    then: [
      'First: the Analysis page, because it exposes settings that have no UI at all.',
      'Then palette rows, then the keyboard map. Debug moves under Advanced; Reset library sits at the bottom of Library as a danger zone.',
    ],
  },
  questions: [
    'Do you want "Analyze new tracks on import" as a toggle (default on) and the "Analyze imported tracks?" dialog gone, or keep asking each time?',
    'Engine (Built-in or Python) and worker count: expose them, or keep them in config for now?',
    'Library folders: the app imports on demand and has no watched folders. Do you want "keep these folders in sync" as a feature, or is that out of scope?',
    'Where should the destructive Reset library live: Library page (danger zone) or Advanced?',
    'Keyboard map: worth building (ANSI/ISO/German layout question), or filter plus two columns is enough?',
    'Sample rate and buffer: show them behind an "Advanced audio" fold, since most people never change them?',
  ],
  kept: [
    'Opened by ⌘, and the status-bar gear; Esc closes.',
    'Rebinding stays click-to-record with Replace / Cancel conflict handling and per-row Reset.',
    'Changing sample rate, buffer or output device stops playback first (existing behaviour).',
  ],
};
