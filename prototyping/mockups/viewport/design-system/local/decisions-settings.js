export const DECISIONS = {
  title: 'Settings: layout overhaul, shortcuts page, analysis backend',
  feedback: [
    { id: 'F21', text: 'Settings need a UI overhaul; today\'s layout is weird. Include the new Keyboard shortcuts page and the analysis backend setting.' },
  ],
  options: [
    {
      tag: 'A',
      name: 'Preferences sheet with a toolbar',
      idea: 'Centred sheet over the library; icon toolbar with six tabs; compact rows. Analysis and Shortcuts tabs shown.',
      board: 'settings/option-a',
    },
    {
      tag: 'B',
      name: 'One searchable settings page',
      idea: 'One scrolling page, search at the top, index on the left, "changed from default" dots. Shortcuts is a section.',
      board: 'settings/option-b',
      rec: true,
    },
    {
      tag: 'C',
      name: 'Settings where they matter',
      idea: 'Audio at the player, analysis at its status chip, shortcuts as a ⌘/ overlay. Preferences shrinks to Library and Advanced.',
      board: 'settings/option-c',
    },
  ],
  matrix: [
    { q: 'Find a setting fast', cells: ['Six tabs, no search', '+Search field plus index', 'Next to the thing, or ⌘K'] },
    { q: 'Keep the library in sight', cells: ['+Sheet over the window', '-Takes over the window (can open as a sheet)', '+Popovers, library stays live'] },
    { q: 'Shortcuts page', cells: ['Tab with filter and conflict row', 'One more section, same table', '+Overlay that doubles as a cheat sheet'] },
    { q: 'Analysis backend', cells: ['Own tab', 'Own section with a dot when changed', '+At the progress chip, where it is relevant'] },
    { q: 'Build effort', cells: ['+Small: one sheet component', '+Small: one page, rows are shared', '-Large: three popovers plus a thin page'] },
    { q: 'Risk', cells: ['Fixed size limits long pages', 'Long page needs scroll-spy', '-Settings scatter; hard to see them all'] },
  ],
  recommendation: {
    pick: 'B as the base, opened as a sheet when it fits (A\'s wrapper), with C\'s audio and analysis popovers added later as shortcuts to the same rows.',
    why: [
      'A pro tool has dozens of settings (audio, analysis, USB, shortcuts) and each new feature adds more; search plus one list scales, a toolbar of tabs does not.',
      'The row component is the same in A and B, so the sheet vs page choice is only a wrapper and can change later.',
      'C is the nicest to use but the most work and leaves no place that shows everything. Add its popovers after the base exists.',
    ],
    then: [
      'Replace the four description-heavy cards with one-line rows (title, one-line hint, control).',
      'Move Reset library and Debug into an "Advanced" section at the bottom, behind the same weight as every other row but with a red button.',
    ],
  },
  questions: [
    'Sheet or full window? Settings today replaces the whole window; a sheet keeps the library visible (useful when tuning audio buffer or analysis).',
    'Analysis engine: show Python vs Built-in to everyone, or hide Python once the built-in one is the default? (Shown here as a two-way switch.)',
    'Should "changed from default" dots and per-row reset exist, or is that too much for a DJ app?',
    'Keep Debug in Settings, or move it behind a hidden shortcut or the command palette?',
  ],
  kept: [
    'Resizable sidebars, ⌘K palette, ⌘, opens settings, Esc closes. Shortcuts registry and its conflict flow (Replace or Cancel).',
  ],
};
