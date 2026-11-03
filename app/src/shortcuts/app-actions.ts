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
import { createFolderAndRename, importIntoSource, revealLabel } from '../menus/context-menus.ts';
import { selection, selectionCount } from '../stores/selection.svelte.ts';
import { isStickSyncBusy } from '../stores/stick-sync.svelte.ts';
import {
  player,
  togglePlay,
  toggleExpanded,
  cueTapPress,
  seekToMs,
  beatGridNudgeLeft,
  beatGridNudgeRight,
  beatGridSetCurrent,
  beatGridShiftByBeat,
  loadPlayerTrack,
  endTrackModes,
} from '../stores/player.svelte.ts';
import { tempo, setKeyShift, setKeyLock, resetTempo } from '../stores/tempo.svelte.ts';
import { zoomWaveIn, zoomWaveOut, zoomWaveToFit } from '../stores/wave-zoom.ts';
import { selectedTrack } from '../stores/selection.svelte.ts';
import { cuesOf } from '../stores/cues.svelte.ts';
import { confirmAction } from '../lib/confirmAction.ts';
import { hasUnanalyzedTracks, revealFile } from '../stores/library.svelte.ts';
import {
  addHotCueAtPlayhead,
  addMemoryCueAtPlayhead,
  clearAllCues,
  deleteHotCue,
  jumpToHotCue,
  triggerHotCue,
} from '../stores/cue-actions.ts';
import { HOT_CUE_SLOTS, firstFreeSlot, hotCueIn, slotLetter } from '../lib/cues.ts';
import { openRekordboxImport } from '../stores/rekordbox-import.svelte.ts';
import { createPlaylistAndOpen } from '../stores/new-playlist.svelte.ts';
import { deleteSidebarSelection, sidebarDeleteReady } from '../stores/sidebar-delete.ts';
import {
  bpmTargetId,
  commitTrackRating,
  doubleTrackBpm,
  halveTrackBpm,
  loadedTrackHasBpm,
  requestBpmEntry,
  stepTrackBpm,
  stepTrackKey,
} from '../stores/track-edits.svelte.ts';
import { announce } from '../stores/announce.svelte.ts';
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
  else if (player.trackBpmMode || player.trackKeyMode) endTrackModes();
  else if (ui.searchText.trim()) clearListFilter();
  else toggleExpanded();
}

async function rateSelection(stars: number) {
  const ids = [...selection.ids];
  const results = await Promise.all(ids.map((id) => commitTrackRating(id, stars)));
  if (!results.every(Boolean)) return;
  const what = stars === 0 ? 'Rating cleared' : `Rated ${stars} ${stars === 1 ? 'star' : 'stars'}`;
  announce(ids.length === 1 ? what : `${what} on ${ids.length} tracks`);
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

const RATING_STEPS = [0, 1, 2, 3, 4, 5];

function listHasFocus(): boolean {
  return document.activeElement?.closest('[role="listbox"]') != null;
}

const ratingActions: ShortcutAction[] = RATING_STEPS.map((stars) => ({
  id: `library.rate${stars}`,
  label: stars === 0 ? 'Clear rating on selection' : `Rate selection ${stars} ${stars === 1 ? 'star' : 'stars'}`,
  group: GROUP.library,
  defaultBinding: `Mod+${stars}`,
  keywords: ['rating', 'stars', 'rate'],
  palette: false,
  inInput: false,
  when: () => selectionCount() > 0 && listHasFocus() && !library.volumes.includes(ui.activeSidebarRow),
  run: () => void rateSelection(stars),
}));

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
      keywords: ['remove', 'clear', 'hot cue', 'cue'],
      when: () => hotCueIn(cuesOf(player.track?.id), slot) !== undefined,
      run: () => deleteHotCue(slot),
    },
    {
      id: `cues.hot${slot}.set`,
      label: `Set hot cue ${letter} at playhead`,
      keywords: ['add', 'new', 'hot cue', 'cue', 'pad'],
      group: GROUP.cues,
      defaultBinding: null,
      when: () => player.track != null && hotCueIn(cuesOf(player.track.id), slot) === undefined,
      run: () => void addHotCueAtPlayhead(slot),
    },
    {
      id: `cues.hot${slot}.jump`,
      label: `Jump to hot cue ${letter}`,
      keywords: ['go', 'seek', 'hot cue', 'cue', 'pad'],
      group: GROUP.cues,
      defaultBinding: null,
      when: () => hotCueIn(cuesOf(player.track?.id), slot) !== undefined,
      run: () => void jumpToHotCue(slot),
    },
  ];
}).flat();

const playerActions: ShortcutAction[] = [
  {
    id: 'playback.start',
    label: 'Jump to track start',
    keywords: ['beginning', 'rewind', 'zero', 'top', 'seek'],
    group: GROUP.playback,
    defaultBinding: null,
    when: () => player.track != null,
    run: () => void seekToMs(0),
  },
  {
    id: 'grid.nudgeLeft',
    label: 'Nudge beat grid left',
    keywords: ['shift', 'earlier', 'beatgrid', 'grid'],
    group: GROUP.playback,
    defaultBinding: null,
    when: () => player.track != null,
    run: () => beatGridNudgeLeft(),
  },
  {
    id: 'grid.nudgeRight',
    label: 'Nudge beat grid right',
    keywords: ['shift', 'later', 'beatgrid', 'grid'],
    group: GROUP.playback,
    defaultBinding: null,
    when: () => player.track != null,
    run: () => beatGridNudgeRight(),
  },
  {
    id: 'grid.setDownbeat',
    label: 'Set downbeat to playhead',
    keywords: ['grid', 'beatgrid', 'one', 'align', 'snap'],
    group: GROUP.playback,
    defaultBinding: null,
    when: () => player.track != null,
    run: () => beatGridSetCurrent(),
  },
  {
    id: 'grid.beatLater',
    label: 'Shift beat grid one beat later',
    keywords: ['grid', 'beatgrid', 'beat', 'edit'],
    group: GROUP.playback,
    defaultBinding: null,
    when: () => player.expanded && player.track != null,
    run: () => beatGridShiftByBeat(1),
  },
  {
    id: 'grid.beatEarlier',
    label: 'Shift beat grid one beat earlier',
    keywords: ['grid', 'beatgrid', 'beat', 'edit'],
    group: GROUP.playback,
    defaultBinding: null,
    when: () => player.expanded && player.track != null,
    run: () => beatGridShiftByBeat(-1),
  },
  {
    id: 'track.key.up',
    label: 'Track key +1 on the Camelot wheel',
    keywords: ['camelot', 'key', 'edit', 'harmonic'],
    group: GROUP.playback,
    defaultBinding: null,
    when: () => player.expanded && player.track != null,
    run: () => void stepTrackKey(player.track!.id, 1),
  },
  {
    id: 'track.key.down',
    label: 'Track key −1 on the Camelot wheel',
    keywords: ['camelot', 'key', 'edit', 'harmonic'],
    group: GROUP.playback,
    defaultBinding: null,
    when: () => player.expanded && player.track != null,
    run: () => void stepTrackKey(player.track!.id, -1),
  },
  {
    id: 'tempo.masterTempo',
    label: 'Toggle master tempo (key lock)',
    keywords: ['mt', 'key lock', 'keylock', 'pitch', 'tempo'],
    group: GROUP.playback,
    defaultBinding: null,
    run: () => setKeyLock(!tempo.keyLock),
  },
  {
    id: 'tempo.keyUp',
    label: 'Shift key up one semitone',
    keywords: ['pitch', 'transpose', 'higher', 'key shift'],
    group: GROUP.playback,
    defaultBinding: null,
    when: () => player.track != null,
    run: () => setKeyShift(tempo.keyShift + 1),
  },
  {
    id: 'tempo.keyDown',
    label: 'Shift key down one semitone',
    keywords: ['pitch', 'transpose', 'lower', 'key shift'],
    group: GROUP.playback,
    defaultBinding: null,
    when: () => player.track != null,
    run: () => setKeyShift(tempo.keyShift - 1),
  },
  {
    id: 'tempo.reset',
    label: 'Reset tempo and key shift',
    keywords: ['pitch', 'speed', 'original', 'zero', 'default'],
    group: GROUP.playback,
    defaultBinding: null,
    when: () => player.track != null && (tempo.percent !== 0 || tempo.keyShift !== 0),
    run: resetTempo,
  },
  {
    id: 'wave.zoomIn',
    label: 'Zoom waveform in',
    keywords: ['magnify', 'closer', 'detail'],
    group: GROUP.view,
    defaultBinding: null,
    when: () => player.expanded && player.track != null,
    run: zoomWaveIn,
  },
  {
    id: 'wave.zoomOut',
    label: 'Zoom waveform out',
    keywords: ['wider', 'overview'],
    group: GROUP.view,
    defaultBinding: null,
    when: () => player.expanded && player.track != null,
    run: zoomWaveOut,
  },
  {
    id: 'wave.zoomFit',
    label: 'Reset waveform zoom to default',
    keywords: ['fit', 'default', 'reset'],
    group: GROUP.view,
    defaultBinding: null,
    when: () => player.expanded && player.track != null,
    run: zoomWaveToFit,
  },
  {
    id: 'library.loadSelected',
    label: 'Load selected track into player',
    keywords: ['open', 'deck', 'cue up'],
    group: GROUP.playback,
    defaultBinding: null,
    when: () => selectedTrack() != null,
    run: () => void loadPlayerTrack(selectedTrack()),
  },
  {
    id: 'library.revealSelected',
    label: revealLabel(),
    keywords: ['finder', 'folder', 'explorer', 'file', 'show', 'reveal'],
    group: GROUP.library,
    defaultBinding: null,
    when: () => !!selectedTrack()?.filePath,
    run: () => void revealFile(selectedTrack()!.filePath),
  },
];

const BPM_KEYWORDS = ['tempo', 'beat grid', 'grid', 'bpm'];

const bpmActions: ShortcutAction[] = [
  {
    id: 'track.bpm.set',
    label: 'Set track BPM…',
    group: GROUP.playback,
    defaultBinding: null,
    keywords: BPM_KEYWORDS,
    when: loadedTrackHasBpm,
    run: () => void requestBpmEntry(),
  },
  {
    id: 'track.bpm.up',
    label: 'Track BPM +0.1',
    group: GROUP.playback,
    defaultBinding: null,
    keywords: BPM_KEYWORDS,
    when: () => bpmTargetId() != null,
    run: () => stepTrackBpm(bpmTargetId()!, 1, 'normal'),
  },
  {
    id: 'track.bpm.down',
    label: 'Track BPM −0.1',
    group: GROUP.playback,
    defaultBinding: null,
    keywords: BPM_KEYWORDS,
    when: () => bpmTargetId() != null,
    run: () => stepTrackBpm(bpmTargetId()!, -1, 'normal'),
  },
  {
    id: 'track.bpm.double',
    label: 'Double track BPM',
    group: GROUP.playback,
    defaultBinding: null,
    keywords: [...BPM_KEYWORDS, '×2', 'x2'],
    when: () => bpmTargetId() != null,
    run: () => doubleTrackBpm(bpmTargetId()!),
  },
  {
    id: 'track.bpm.halve',
    label: 'Halve track BPM',
    group: GROUP.playback,
    defaultBinding: null,
    keywords: [...BPM_KEYWORDS, '÷2', 'half'],
    when: () => bpmTargetId() != null,
    run: () => halveTrackBpm(bpmTargetId()!),
  },
];

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
      label: 'Add memory cue at playhead',
      keywords: ['marker', 'cue', 'locator', 'add'],
      group: GROUP.cues,
      defaultBinding: 'M',
      when: () => player.track != null,
      run: addMemoryCueAtPlayhead,
    },
    {
      id: 'cues.hot.add',
      label: 'Add hot cue at playhead (next free pad)',
      keywords: ['new', 'hot cue', 'cue', 'pad', 'set'],
      group: GROUP.cues,
      defaultBinding: 'Shift+C',
      when: () => player.track != null && firstFreeSlot(cuesOf(player.track.id)) !== null,
      run: () => void addHotCueAtPlayhead(),
    },
    {
      id: 'cues.clearAll',
      label: 'Clear all cues',
      keywords: ['delete', 'remove', 'hot cue', 'memory', 'reset'],
      group: GROUP.cues,
      defaultBinding: null,
      when: () => player.track != null && cuesOf(player.track.id).length > 0,
      run: () =>
        void confirmAction('Delete every cue on the loaded track?').then((ok) => {
          if (ok) clearAllCues();
        }),
    },
    ...hotCueActions,
    ...playerActions,
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
      when: () => !library.syncing && hasUnanalyzedTracks(),
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
      id: 'library.playlists.delete',
      label: 'Delete selected playlists',
      group: GROUP.library,
      defaultBinding: 'Backspace',
      palette: false,
      when: sidebarDeleteReady,
      run: () => void deleteSidebarSelection(),
    },
    {
      id: 'library.playlists.deleteForward',
      label: 'Delete selected playlists (forward delete)',
      group: GROUP.library,
      defaultBinding: 'Delete',
      palette: false,
      when: sidebarDeleteReady,
      run: () => void deleteSidebarSelection(),
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
    ...bpmActions,
    ...favoriteActions,
    ...ratingActions,
  ];
}
