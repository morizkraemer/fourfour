export const DECISIONS = {
  title: 'Inspector: every field editable, new layout',
  feedback: [
    { id: 'F22', text: 'Every field editable (title, artist, album, label, BPM, key, comment, rating, colour, cues…) plus a layout overhaul. Today: big cover, waveform strip, label/value list, hot cue chips truncated to "H..", the file path.' },
  ],
  options: [
    {
      tag: 'A',
      name: 'Live form panel',
      idea: 'Same right-hand panel rebuilt as a form of ghost inputs. Scrub fields for BPM and key. Cues as an editable list. Small cover, no waveform.',
      board: 'inspector/option-a',
    },
    {
      tag: 'B',
      name: 'Tabbed panel, cues on the waveform',
      idea: 'BPM, key and rating always visible. Info, Cues, Grid, File as tabs. Cues are edited by dragging flags on a mini waveform.',
      board: 'inspector/option-b',
    },
    {
      tag: 'C',
      name: 'Wide editor strip',
      idea: 'Inspector spreads under the table: all fields, a Camelot key wheel and a full-width cue lane, no tabs, no scrolling. Pairs with main-window option B.',
      board: 'inspector/option-c',
      rec: true,
    },
  ],
  matrix: [
    { q: 'All fields visible without scrolling', cells: ['-No, column scrolls on short windows', '-Rare fields hide in tabs', '+Yes'] },
    { q: 'Cue editing', cells: ['-List: type or nudge a time', '+Drag flags on a mini waveform', '+Drag flags on a full-width waveform'] },
    { q: 'BPM / key edit', cells: ['Scrub field, type, ↑↓', 'Scrub field, type, ↑↓', '+Big scrub field, key on a Camelot wheel'] },
    { q: 'Costs table space', cells: ['-360px of width', '-360px of width', '-~310px of height (collapsible)'] },
    { q: 'Multi-select (batch edit)', cells: ['+Fields show "Mixed", easy', '+Same; Cues tab disabled', 'Needs its own design'] },
    { q: 'Change from today', cells: ['+Smallest', 'Medium', '-Biggest (moves the panel)'] },
  ],
  recommendation: {
    pick: 'C if the main-window decision is B, otherwise B. The edit model (ghost inputs, scrub fields, flags on a waveform) is shared and can be built once.',
    why: [
      'Ghost inputs plus scrub fields are the answer to "every field editable" in all three options: it looks like text until you touch it, so the panel stays calm.',
      'Cue editing is the case that needs a waveform. Lists with typed times (A) are slow; flags you drag and snap to the grid (B, C) are how a DJ thinks about cues.',
      'C is the only layout that shows everything at once. It also gives the Camelot wheel room, which doubles as a "what mixes with this" hint.',
    ],
    then: [
      'Build the field primitives first (ghost input, scrub field, rating stars, colour chips): they are needed in every option and in the table.',
      'Autosave to the library on commit; writing back to file tags is a separate, explicit setting (see question 1).',
    ],
  },
  questions: [
    'Do edits write to the audio file tags, only to the library, or ask? A "Saved" label is shown in every option; the meaning is not decided.',
    'Cue editing: here, in the expanded player, or both? B and C duplicate what the player will have.',
    'Multi-select: should the inspector become a batch editor (fields show "Mixed"), as in every other DJ tool?',
    'Do you want the file path and format in the inspector at all, or only in a context menu?',
  ],
  kept: [
    'Left = browse, right = put. Simple row previews, 3-band players, stick header, ⌘K palette.',
  ],
};
