export const DECISIONS = {
  title: 'Inspector round 3: calmer, one clear hierarchy (F22)',
  feedback: [
    { id: 'F22', text: 'Round 2 was too overwhelming, no clear hierarchy, and the key ring was disliked. Round 3: one primary block (track), then BPM, key, length, then quiet metadata. Fewer borders, boxes and colours. Every field still editable, on click, and plain text at rest. No Camelot wheel.' },
    { id: 'Lock', text: 'Hand-set BPM, key, grid or cues show as one small padlock, not a badge per field.' },
  ],
  options: [
    {
      tag: 'A',
      name: 'Document',
      idea: 'One quiet column in order of importance: cover and title, three figures, one lock line, plain tag rows, short cue list, file path.',
      board: 'inspector-3/option-a',
    },
    {
      tag: 'B',
      name: 'Hero and folds',
      idea: 'Cover as a hero with the title on it, one strip of BPM, key, length and a padlock, then folds: Cues open, Details and File closed with a summary line.',
      board: 'inspector-3/option-b',
      rec: true,
    },
    {
      tag: 'C',
      name: 'Waveform spine',
      idea: 'Title, figures, then a waveform with cue flags and the cue list under it. Tags shrink into one quiet line, then stars, colour, comment.',
      board: 'inspector-3/option-c',
    },
  ],
  matrix: [
    { q: 'Calm at rest', cells: ['Medium: all fields visible', '+Calmest: three blocks', '+Calm: short, one picture'] },
    { q: 'DJ essentials at a glance', cells: ['+Yes', '+Yes', '+Yes'] },
    { q: 'Tags editable', cells: ['+All visible, one click', '-One click to open Details first', '-In a line, small targets'] },
    { q: 'Cues', cells: ['Typed rows', 'Typed rows', '+Rows plus a waveform'] },
    { q: 'Fits 900 px without scrolling', cells: ['-Tight', '+Yes', '+Yes'] },
    { q: 'New work beyond ghost fields', cells: ['+None', '+Fold headers', '-Waveform strip in the panel'] },
  ],
  recommendation: {
    pick: 'B: hero and folds. Open Details only when you edit tags; the rest of the time the panel is a cover, three figures and a cue list.',
    why: [
      'It is the calmest at rest and the hierarchy is the layout: image and title, then the figures, then folds.',
      'Summaries on closed folds answer most glances without opening anything.',
      'It stays short on any window height, so cues never fall off the screen.',
    ],
    then: [
      'If tags are edited often, start with Details open (A is that case).',
      'The waveform in C can be added to the Cues fold later.',
    ],
  },
  questions: [
    'Should Details start open or closed?',
    'Is one padlock for BPM and grid enough, or do you need to see which of them is hand-set (tooltip only)?',
    'Cue times: typed is fine for now, or is a waveform a must?',
    'Cover as a hero at 168 px: OK, or smaller?',
  ],
  kept: [
    'Placement: right of the open list inside the main view; hidden while the right sidebar is open.',
    'Edits go to the library only. ScrubValue for BPM and key, in the Camelot table colour.',
  ],
};
