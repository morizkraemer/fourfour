export const DECISIONS = {
  title: 'Inspector round 2: every field editable, locks visible (F22)',
  feedback: [
    { id: 'F22', text: 'Every field editable (title, artist, album, genre, label, year, comment, rating, colour, BPM, key, grid actions, cues) and a layout overhaul. Placement is fixed: the inspector stays on the right of the open track list. Edits go to the library only, never to file tags.' },
    { id: 'Lock', text: 'BPM, key, grid and cues carry a lock in the library (bpm_locked, key_locked, grid_locked, cues_locked in edits.json). Re-analysis leaves locked values alone. Today only a right-click item mentions it. Rekordbox-imported grids and cues are locked the same way.' },
  ],
  options: [
    {
      tag: 'A',
      name: 'Form column of ghost fields',
      idea: 'One scrolling column. Every value is text until touched (pencil on hover, input on click). BPM and key are the real ScrubValue. Cues are rows with name and time fields. Lock sits next to each value.',
      board: 'inspector-2/option-a',
    },
    {
      tag: 'B',
      name: 'Tiles and focused editors',
      idea: 'Compact read-first card with six tiles. Each tile opens the editor that fits: Camelot wheel for key, menu for grid, CueEditor plus a time row for cues. A padlock on each tile.',
      board: 'inspector-2/option-b, option-b-cue',
    },
    {
      tag: 'C',
      name: 'Two jobs: Prep and Tags',
      idea: 'A two-way switch. Prep: tempo, key, grid and cues around an overview plus a zoomed waveform lane where cues are dragged. Tags: every text field, rating, colour and the read-only file block.',
      board: 'inspector-2/option-c, option-c-tags',
      rec: true,
    },
  ],
  matrix: [
    { q: 'Edit title, artist, album, genre, label, year, comment', cells: ['+Inline, one click', 'Inline for short fields', '+Inline on the Tags side'] },
    { q: 'Rating and colour', cells: ['Inline stars and chips', 'Tiles, one click', 'Inline stars and chips'] },
    { q: 'Cue time and name', cells: ['-Typed in rows, no picture', '-Two clicks, nudge by beat', '+Drag on a zoomed waveform, snaps to beats'] },
    { q: 'Grid actions (shift, ×2, ½, fit, re-detect)', cells: ['Buttons in a section', 'Menu on the grid tile', '+Buttons next to the waveform'] },
    { q: 'Lock state', cells: ['Yours chip plus analysed value and "Use"', 'Padlock on every tile, open = analysed', '+Yours / Analysed per group, analysed value beside yours'] },
    { q: 'Fits a 900 px window without scrolling', cells: ['-No, cues and file fall below the fold', '+Yes', '+Yes, on both sides'] },
    { q: 'New work beyond GhostField and LockMark', cells: ['+None', 'Key wheel popover, grid menu', '-Zoomable waveform in a side panel'] },
  ],
  recommendation: {
    pick: 'C: split the inspector by job. Prep (tempo, key, grid, cues on a waveform) and Tags (everything typed). Ghost fields from A on the Tags side, B\'s key wheel as an option on the key scrub.',
    why: [
      'Fixing a cue or a grid is a picture problem. A typed time is slow and blind; a flag you drag on a zoomed lane, snapped to the beat, is how a DJ thinks.',
      'The lock needs room to explain itself. With the analysed value next to yours and one click to hand it back, the state is understood without a manual.',
      'Typing tags and checking a grid are different moods. Keeping them apart means rare fields never push cues off the screen.',
    ],
    then: [
      'GhostField and LockMark are already in the design system; build the Tags side first (cheap, no waveform work).',
      'Prep follows with the zoomed lane. If that is too big for now, ship A\'s cue rows plus beat nudges and keep C as the target.',
      'Multi-select: fields show "Mixed" and write to all; Prep is disabled with a one-line note.',
    ],
  },
  questions: [
    'Is a padlock you can click without editing wanted (lock a good analysis so re-analysis never touches it), or does a lock only come from your own edit?',
    'Lock granularity: today it is four flags (BPM, key, grid, cues) and "release" frees all four. Should the UI release one at a time?',
    'Rekordbox-imported grids and cues are locked too. Show them as "Imported" (a third origin) or just "Yours"?',
    'Cue time precision: snap to beats by default with Alt for free, or free by default?',
    'Do you want Prep to share its waveform with the expanded player, or stay a separate small view?',
    'Panel width: 340 to 380 px default (today 278). OK to widen, and does the list still feel big enough at 1280?',
    'Multi-select: batch edit of tags only (Mixed), or also BPM and key offsets?',
  ],
  kept: [
    'Placement: right of the open list inside the main view; the organize sidebar stays at the window edge.',
    'Edits are written to the library only. A line on the Tags side says so.',
    'ScrubValue for BPM and key (drag, wheel, type, menu); a BPM change never moves the grid, "Fit" is a second, explicit step.',
  ],
};
