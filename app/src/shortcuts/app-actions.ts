import {
  ui,
  togglePalette,
  openSettings,
  toggleSettings,
  closeSettings,
  openPalette,
  clearListFilter,
  toggleDetailPane,
  toggleOrganize,
  importDropTarget,
} from '../stores/ui.svelte.ts';
import {
  library,
  toggleFavoriteSlot,
  analyzeTrackIds,
  syncAllSticks,
  syncToUsb,
  analyzeTracks,
  refreshVolumes,
  FAVORITE_SLOTS,
} from '../stores/library.svelte.ts';
import { createFolderAndRename, importIntoSource } from '../menus/context-menus.ts';
import { selection, selectionCount } from '../stores/selection.svelte.ts';
import { isStickSyncBusy } from '../stores/stick-sync.svelte.ts';
import { player, togglePlay, toggleExpanded, cueTapPress } from '../stores/player.svelte.ts';
import { addMemoryCueAtPlayhead, deleteHotCue, triggerHotCue } from '../stores/cue-actions.ts';
import { HOT_CUE_SLOTS, slotLetter } from '../lib/cues.ts';
import { openRekordboxImport } from '../stores/rekordbox-import.svelte.ts';
import { createPlaylistAndOpen } from '../stores/new-playlist.svelte.ts';
import { focusRegion } from './focus-regions.ts';
import type { ShortcutAction } from './registry.svelte.ts';

const GROUP = {
  general: 'General',
  playback: 'Playback',
  library: 'Library',
  view: 'View',
  cues: 'Cues',
  favorites: 'Favorites',
} as const;

function dismissibleLayerOpen(): boolean {
  return !library.showAnalysisPrompt && (ui.settingsOpen || ui.searchText.trim() !== '' || player.expanded);
}

function noOverlayOpen(): boolean {
  return !ui.settingsOpen && !ui.paletteOpen && !library.showAnalysisPrompt;
}

function dismissTopLayer() {
  if (ui.settingsOpen) closeSettings();
  else if (ui.searchText.trim()) clearListFilter();
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

const hotCueActions: ShortcutAction[] = Array.from({ length: HOT_CUE_SLOTS }, (_, i) => {
  const slot = i + 1;
  const letter = slotLetter(slot);
  return [
    {
      id: `cues.hot${slot}`,
      label: `Hot cue ${letter}: set at playhead or jump`,
      group: GROUP.cues,
      defaultBinding: `Shift+${slot}`,
      palette: false,
      when: () => player.track != null,
      run: () => void triggerHotCue(slot),
    },
    {
      id: `cues.hot${slot}.delete`,
      label: `Delete hot cue ${letter}`,
      group: GROUP.cues,
      defaultBinding: `Alt+Shift+${slot}`,
      palette: false,
      when: () => player.track != null,
      run: () => deleteHotCue(slot),
    },
  ];
}).flat();

export function buildAppActions(): ShortcutAction[] {
  return [
    {
      id: 'palette.toggle',
      label: 'Search and commands',
      group: GROUP.general,
      defaultBinding: 'Mod+K',
      palette: false,
      run: togglePalette,
    },
    {
      id: 'palette.open',
      label: 'Search and commands (alternate)',
      group: GROUP.general,
      defaultBinding: 'Alt+Enter',
      palette: false,
      inInput: true,
      run: openPalette,
    },
    {
      id: 'settings.toggle',
      label: 'Open settings',
      keywords: ['preferences', 'options', 'config', 'prefs'],
      group: GROUP.general,
      defaultBinding: 'Mod+,',
      run: toggleSettings,
    },
    {
      id: 'settings.shortcuts',
      label: 'Show keyboard shortcuts',
      keywords: ['keyboard', 'keys', 'bindings', 'hotkeys'],
      group: GROUP.general,
      defaultBinding: null,
      run: () => openSettings('keyboard'),
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
      keywords: ['play', 'pause', 'stop', 'start', 'transport'],
      group: GROUP.playback,
      defaultBinding: 'Space',
      when: () => player.track != null,
      run: () => void togglePlay(),
    },
    {
      id: 'cues.cue',
      label: 'Cue: return to cue point, or set it at the playhead',
      group: GROUP.cues,
      defaultBinding: 'C',
      when: () => player.track != null,
      run: () => void cueTapPress(),
    },
    {
      id: 'cues.memory.add',
      label: 'Add cue at playhead',
      keywords: ['memory', 'marker', 'cue', 'locator'],
      group: GROUP.cues,
      defaultBinding: 'M',
      when: () => player.track != null,
      run: addMemoryCueAtPlayhead,
    },
    ...hotCueActions,
    {
      id: 'library.search',
      label: 'Search tracks',
      group: GROUP.library,
      defaultBinding: 'Mod+F',
      palette: false,
      run: openPalette,
    },
    {
      id: 'library.newPlaylist',
      label: 'New playlist',
      keywords: ['playlist', 'create', 'crate', 'add'],
      group: GROUP.library,
      defaultBinding: 'Mod+Shift+N',
      run: () => void createPlaylistAndOpen(),
    },
    {
      id: 'library.import',
      label: 'Import folder…',
      keywords: ['add', 'music', 'files', 'tracks', 'scan'],
      group: GROUP.library,
      defaultBinding: 'Mod+O',
      inInput: false,
      when: () => !library.analyzing && !library.syncing,
      run: () => void importIntoSource(importDropTarget()),
    },
    {
      id: 'library.analyze',
      label: 'Analyze selection',
      keywords: ['bpm', 'key', 'waveform', 'detect', 'selected'],
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
      palette: false,
      when: () => library.activeVolume != null && !library.syncing && !isStickSyncBusy(),
      run: () => void syncToUsb(),
    },
    {
      id: 'library.syncAll',
      label: 'Sync all sticks',
      group: GROUP.library,
      defaultBinding: null,
      keywords: ['usb', 'stick', 'export', 'write', 'every', 'drives'],
      when: () => library.volumes.length > 0 && !library.syncing && !isStickSyncBusy(),
      run: () => void syncAllSticks(),
    },
    {
      id: 'library.analyzeAll',
      label: 'Analyze all unanalyzed tracks',
      group: GROUP.library,
      defaultBinding: null,
      keywords: ['bpm', 'key', 'waveform', 'detect', 'everything'],
      when: () => !library.syncing && library.tracks.some((t) => !t.analyzed),
      run: () => void analyzeTracks(),
    },
    {
      id: 'library.importRekordbox',
      label: 'Import from Rekordbox…',
      group: GROUP.library,
      defaultBinding: null,
      keywords: ['pioneer', 'xml', 'database', 'migrate', 'playlists', 'cues'],
      when: () => !library.analyzing && !library.syncing,
      run: () => void openRekordboxImport(),
    },
    {
      id: 'library.newFolder',
      label: 'New folder',
      group: GROUP.library,
      defaultBinding: null,
      keywords: ['playlist', 'create', 'group', 'directory'],
      run: () => void createFolderAndRename(0),
    },
    {
      id: 'library.refreshDevices',
      label: 'Refresh devices',
      group: GROUP.library,
      defaultBinding: null,
      keywords: ['usb', 'stick', 'drive', 'rescan', 'mount', 'detect'],
      run: () => void refreshVolumes(),
    },
    {
      id: 'view.player',
      label: 'Toggle big / small player',
      keywords: ['expand', 'collapse', 'waveform', 'big', 'small'],
      group: GROUP.view,
      defaultBinding: 'Tab',
      when: () => noOverlayOpen() && player.track != null,
      run: toggleExpanded,
    },
    {
      id: 'view.organize',
      label: 'Toggle organize drawer',
      keywords: ['sidebar', 'drawer', 'panel', 'stick board'],
      group: GROUP.view,
      defaultBinding: 'Shift+Tab',
      when: noOverlayOpen,
      run: toggleOrganize,
    },
    {
      id: 'view.inspector',
      label: 'Toggle inspector',
      keywords: ['detail', 'info', 'panel', 'sidebar'],
      group: GROUP.view,
      defaultBinding: null,
      run: toggleDetailPane,
    },
    ...favoriteActions,
  ];
}
