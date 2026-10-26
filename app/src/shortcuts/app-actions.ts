import {
  ui,
  togglePalette,
  openSettings,
  toggleSettings,
  closeSettings,
  focusListFilter,
  clearListFilter,
  toggleDetailPane,
  toggleOrganize,
  importDropTarget,
} from '../stores/ui.svelte.ts';
import {
  library,
  toggleFavoriteSlot,
  analyzeTrackIds,
  syncToUsb,
  FAVORITE_SLOTS,
} from '../stores/library.svelte.ts';
import { importIntoSource } from '../menus/context-menus.ts';
import { selection, selectionCount } from '../stores/selection.svelte.ts';
import { player, togglePlay, toggleExpanded } from '../stores/player.svelte.ts';
import { createPlaylistAndOpen } from '../stores/new-playlist.svelte.ts';
import { focusRegion } from './focus-regions.ts';
import type { ShortcutAction } from './registry.svelte.ts';

const GROUP = {
  general: 'General',
  playback: 'Playback',
  library: 'Library',
  view: 'View',
  favorites: 'Favorites',
} as const;

function dismissibleLayerOpen(): boolean {
  return !library.showAnalysisPrompt && (ui.settingsOpen || ui.listFilter.trim() !== '' || player.expanded);
}

function noOverlayOpen(): boolean {
  return !ui.settingsOpen && !ui.paletteOpen && !library.showAnalysisPrompt;
}

function dismissTopLayer() {
  if (ui.settingsOpen) closeSettings();
  else if (ui.listFilter.trim()) clearListFilter();
  else toggleExpanded();
}

const favoriteActions: ShortcutAction[] = Array.from({ length: FAVORITE_SLOTS }, (_, i) => {
  const slot = i + 1;
  return {
    id: `favorites.toggle${slot}`,
    label: `Toggle favorite ${slot} on selection`,
    group: GROUP.favorites,
    defaultBinding: String(slot),
    palette: false,
    when: () => selectionCount() > 0,
    run: () => void toggleFavoriteSlot(slot, [...selection.ids]),
  };
});

export function buildAppActions(): ShortcutAction[] {
  return [
    {
      id: 'palette.toggle',
      label: 'Command palette',
      group: GROUP.general,
      defaultBinding: 'Mod+K',
      palette: false,
      run: togglePalette,
    },
    {
      id: 'settings.toggle',
      label: 'Open settings',
      group: GROUP.general,
      defaultBinding: 'Mod+,',
      run: toggleSettings,
    },
    {
      id: 'settings.shortcuts',
      label: 'Show keyboard shortcuts',
      group: GROUP.general,
      defaultBinding: null,
      run: () => openSettings('shortcuts'),
    },
    {
      id: 'ui.dismiss',
      label: 'Close settings, clear search, or collapse player',
      group: GROUP.general,
      defaultBinding: 'Escape',
      palette: false,
      when: dismissibleLayerOpen,
      run: dismissTopLayer,
    },
    {
      id: 'focus.next',
      label: 'Focus next region',
      group: GROUP.general,
      defaultBinding: 'F6',
      run: () => focusRegion(1),
    },
    {
      id: 'focus.previous',
      label: 'Focus previous region',
      group: GROUP.general,
      defaultBinding: 'Shift+F6',
      run: () => focusRegion(-1),
    },
    {
      id: 'playback.toggle',
      label: 'Play / pause',
      group: GROUP.playback,
      defaultBinding: 'Space',
      when: () => player.track != null,
      run: () => void togglePlay(),
    },
    {
      id: 'library.search',
      label: 'Search tracks',
      group: GROUP.library,
      defaultBinding: 'Mod+F',
      run: focusListFilter,
    },
    {
      id: 'library.newPlaylist',
      label: 'New playlist',
      group: GROUP.library,
      defaultBinding: 'Mod+Shift+N',
      run: () => void createPlaylistAndOpen(),
    },
    {
      id: 'library.import',
      label: 'Import files…',
      group: GROUP.library,
      defaultBinding: 'Mod+O',
      inInput: false,
      when: () => !library.analyzing && !library.syncing,
      run: () => void importIntoSource(importDropTarget()),
    },
    {
      id: 'library.analyze',
      label: 'Analyze selection',
      group: GROUP.library,
      defaultBinding: null,
      when: () => selectionCount() > 0 && !library.analyzing,
      run: () => void analyzeTrackIds([...selection.ids]),
    },
    {
      id: 'library.sync',
      label: 'Sync stick',
      group: GROUP.library,
      defaultBinding: null,
      when: () => library.activeVolume != null && !library.syncing,
      run: () => void syncToUsb(),
    },
    {
      id: 'view.player',
      label: 'Toggle big / small player',
      group: GROUP.view,
      defaultBinding: 'Tab',
      when: () => noOverlayOpen() && player.track != null,
      run: toggleExpanded,
    },
    {
      id: 'view.organize',
      label: 'Toggle organize drawer',
      group: GROUP.view,
      defaultBinding: 'Shift+Tab',
      when: noOverlayOpen,
      run: toggleOrganize,
    },
    {
      id: 'view.inspector',
      label: 'Toggle inspector',
      group: GROUP.view,
      defaultBinding: null,
      run: toggleDetailPane,
    },
    ...favoriteActions,
  ];
}
