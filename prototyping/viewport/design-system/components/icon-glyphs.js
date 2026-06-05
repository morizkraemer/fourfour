/*
 * Icon glyph geometry (lucide, 24×24 viewBox), shared by the Icon component and
 * any consumer that needs the raw inner SVG. Single source for the glyph set.
 */
export const ICONS = {
  list: '<path d="M8 6h13"/><path d="M8 12h13"/><path d="M8 18h13"/><path d="M3 6h.01"/><path d="M3 12h.01"/><path d="M3 18h.01"/>',
  'clock-3': '<circle cx="12" cy="12" r="10"/><path d="M12 6v6h4.5"/>',
  plus: '<path d="M5 12h14"/><path d="M12 5v14"/>',
  'folder-open':
    '<path d="m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2"/>',
  'chevron-down': '<path d="m6 9 6 6 6-6"/>',
  'chevron-left': '<path d="m15 18-6-6 6-6"/>',
  'chevron-right': '<path d="m9 18 6-6-6-6"/>',
  search: '<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>',
  x: '<path d="M18 6 6 18"/><path d="m6 6 12 12"/>',
  eject: '<path d="M5 16 12 6l7 10z"/><path d="M5 19h14"/>',
  disc: '<circle cx="12" cy="12" r="10"/><circle cx="12" cy="12" r="2"/>',
  star: '<path d="M11.48 3.5a.6.6 0 0 1 1.04 0l2.45 4.97 5.48.8a.6.6 0 0 1 .33 1.02l-3.96 3.86.94 5.46a.6.6 0 0 1-.87.63L12 17.98l-4.9 2.57a.6.6 0 0 1-.87-.63l.94-5.46-3.97-3.86a.6.6 0 0 1 .33-1.02l5.48-.8z"/>',
  // open-in-side-panel (split list into two columns) — lucide columns-2
  'columns-2': '<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M12 3v18"/>',
  // collapse-right-panel — lucide panel-right
  'panel-right': '<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M15 3v18"/>',
  // settings gear — lucide settings
  settings: '<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/>',
};

export const ICON_NAMES = Object.keys(ICONS);
