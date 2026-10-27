export const DECISIONS = {
  title: 'Organize drawer: empty state and header',
  feedback: [
    { id: 'F15', text: 'The organize sidebar empty state is very bad: a huge empty area, "New playlist" floating mid-panel.' },
    { id: 'F16', text: 'The drawer header\'s "panel" icon vs "x" close is confusing.' },
  ],
  options: [
    {
      tag: 'A',
      name: 'Destination cards',
      idea: 'Top-anchored picker made of cards: recent playlists, a New playlist card, sticks with capacity. Click opens; dropping on a card puts tracks there.',
      board: 'organize-sidebar/option-a',
      rec: true,
    },
    {
      tag: 'B',
      name: 'Drop to start',
      idea: 'The header is the picker (combobox); the empty body is one big drop zone that creates a named playlist on drop.',
      board: 'organize-sidebar/option-b',
    },
    {
      tag: 'C',
      name: 'Set builder',
      idea: 'The board is an ordered set. Empty: start with the selected track plus compatible next tracks. Filled: transitions between tracks.',
      board: 'organize-sidebar/option-c',
    },
  ],
  matrix: [
    { q: 'Empty state is useful', cells: ['+Yes: every destination one click away', '+Yes, for the main job: just drop', '+Yes: suggestions from the selection'] },
    { q: 'Sticks', cells: ['+First-class cards, capacity, Synced', '-Only in the combobox', 'Not shown (set builder is playlists)'] },
    { q: 'Header: panel icon vs x (F16)', cells: ['+Widen and Hide, labelled; clear is a menu item', '+Same', '+Same'] },
    { q: 'Scales to 30+ playlists', cells: ['Search field first, recents as cards', '+Combobox with search', '+Not about browsing'] },
    { q: 'Surprise risk', cells: ['+Low', '-Implicit playlists on drop', '-Assumes order matters'] },
    { q: 'Build effort', cells: ['Medium: cards, mosaic cover', '+Small: drop zone and combobox', '-Large: suggestions and transitions'] },
  ],
  recommendation: {
    pick: 'A for the empty state, using B\'s trick (drop anywhere on the empty drawer creates a playlist) as a fallback. C is a separate feature, worth its own round.',
    why: [
      'A fixes F15 directly: nothing floats, everything is top-anchored, and sticks (the point of the product) get the room they deserve.',
      'Dropping tracks on the empty drawer should always do something. In A the cards are the targets; dropping on blank space can create "Set 26 Oct" like B.',
      'F16: remove the x. The two remaining actions are verbs people understand: Widen to full window, Hide (chevron). Clearing the target is "Clear board" in the header menu.',
      'C is attractive but is a new product idea (ordered sets with transitions), not a fix for an empty state. Keep as a follow-up.',
    ],
    then: [
      'Use the same two header verbs in the main-window docks (see main-window options).',
      'Keep the Synced mark only on playlists that follow the library; Keep updated wording stays.',
    ],
  },
  questions: [
    'Should dropping on blank drawer space create a playlist (B) or do nothing and show a hint?',
    'Recents or favourites? The cards show the four most used; do you want to pin some?',
    'The "Hide" chevron collapses to a rail today. Keep the rail, or hide the drawer completely (top bar toggle) as in main-window option A?',
    'Is set ordering with transition hints (C) something you want on the roadmap?',
  ],
  kept: [
    'Left = browse, right = put. The board is a sticky target; drop line shows the insert position. Resizable sidebars. Stick header, Synced / Keep updated.',
  ],
};
