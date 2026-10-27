export const DECISIONS = {
  title: 'Main window: top bar and the right-hand system',
  feedback: [
    { id: 'F23', text: '"The main window is still a bit weird." What belongs in the top bar? How should inspector and right sidebar work together?' },
    { id: 'F16', text: 'The drawer header has a "panel" icon and an "x" that look the same. What do they do?' },
  ],
  options: [
    {
      tag: 'A',
      name: 'One right dock, two tabs',
      idea: 'Track and Board are tabs of one dock. One toggle in the top bar shows or hides the dock; headers have no close icons. Top bar: context title, filter, status chips.',
      board: 'main-window/option-a',
    },
    {
      tag: 'B',
      name: 'Track strip below, board right',
      idea: 'Left browses, right puts, bottom shows the track. Inspector becomes a full-width editor strip under the table. Top bar is a filter bar (BPM, key, rating chips).',
      board: 'main-window/option-b',
      rec: true,
    },
    {
      tag: 'C',
      name: 'Inline row editor, board right',
      idea: 'The selected row expands into a compact editor. The right side is only the board. Top bar is one omnibox (⌘K) plus status chips.',
      board: 'main-window/option-c',
    },
  ],
  matrix: [
    { q: 'Can I see the track and the board together?', cells: ['-No, one tab at a time', '+Yes, always', '+Yes, always'] },
    { q: 'Room for the library table (30k rows, many columns)', cells: ['+Dock 360px; reflow needed', '-Strip takes ~330px of height (collapsible)', '-Rows shift when a row expands'] },
    { q: 'How much does the inspector get', cells: ['-Narrow vertical form, scrolls', '+Full width, nothing scrolls, cues on waveform', '-Fixed compact form, chips only for cues'] },
    { q: 'Learning cost', cells: ['+Lowest: one dock, one toggle', 'Three places, each with one job', '+Lowest: no extra panel'] },
    { q: 'F16: panel icon vs x', cells: ['+Removed: top bar toggle is the only way to hide', '+Two labelled verbs: Widen, Hide; clear is in the target menu', '+Same as B'] },
    { q: 'What the top bar does', cells: ['Context + status + dock toggle', 'Filter chips for triage', 'Omnibox: ⌘F and ⌘K merged'] },
    { q: 'Build effort', cells: ['+Small: tab bar over two existing panels', '-Large: new inspector layout, player boundary', 'Medium: expanded-row component'] },
  ],
  recommendation: {
    pick: 'B for the layout, with A\'s single dock toggle and C\'s omnibox kept as options for the top bar.',
    why: [
      'The workflow is "look at this track, decide, put it on the board". B is the only option where all three are on screen together.',
      'The inspector gets a real home: every field is editable without scrolling, cues are edited on a waveform.',
      'Left = browse, right = put, bottom = the track. Each region has one job, and the vision doc\'s "everything hideable" is met by a chevron on the strip and one toggle for the board.',
      'F16 is resolved with words, not icons: Widen and Hide are labelled by tooltip and ⇧⇥; clearing the board is a menu item, not a second close button.',
    ],
    then: [
      'Ship the dock toggle and the removal of the cover rail first (A\'s change), it is small and independent.',
      'Then the strip. If the strip feels too tall at 1280, fall back to A\'s dock for narrow windows.',
    ],
  },
  questions: [
    'Do you want the track strip always on, or only when you ask (⌘I)? Always-on costs ~330px of table height on every screen.',
    'Is the top bar for filtering (B) or for finding anything (C, omnibox)? One could be a mode of the other: typing in the omnibox could offer "add filter".',
    'Should status chips (analysis progress, stick sync) live in the top bar, or stay in the status bar at the bottom where they are today?',
    'Cues: edit them on the strip (B) or only in the expanded player? The review doc recommended the player. B makes both possible.',
  ],
  kept: [
    'Left = browse, right = put/board. Stick header design, Synced / "Keep updated" wording, simple row previews, 3-band players, ⌘K palette, resizable sidebars.',
  ],
};
