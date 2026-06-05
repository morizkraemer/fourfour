/**
 * Shared context-menu item builders — keep all app menus on the $ds ContextMenu shape.
 */

import {
  library,
  playlistByName,
  favoritePlaylistName,
  favoriteSlotFromName,
  canRemoveFavoriteSlot,
  isFavoritePlaylistName,
  toggleFavoriteSlot,
  moveTracksToPlaylist,
  analyzeTrackIds,
  analyzeTracks,
  removeTracks,
  removeTracksFromPlaylist,
  clearPlaylistTracks,
  deletePlaylist,
  importDirectory,
  syncToUsb,
  wipeVolume,
  ejectVolume,
  selectVolume,
  refreshVolumes,
  setTestCuesOnSelected,
  FAVORITE_SLOTS,
} from '../stores/library.svelte.ts';
import { pickDirectory } from '../services/tauri.svelte.ts';
import { openSplitPanel } from '../stores/ui.svelte.ts';

/**
 * @typedef {object} MenuItem
 * @property {string} [label]
 * @property {string} [shortcut]
 * @property {'default'|'danger'|'disabled'} [variant]
 * @property {boolean} [disabled]
 * @property {boolean} [checked]
 * @property {boolean} [keepOpen]
 * @property {boolean} [isSeparator]
 * @property {MenuItem[]} [items]
 * @property {() => void} [onClick]
 */

const LIBRARY_VIEWS = new Set(['All Tracks', 'Recently Added']);

function busy() {
  return library.analyzing || library.syncing;
}

/** @param {() => void} [onSplit] */
function openSplitItem(sourceId, label, onSplit) {
  return {
    label: 'Open in side panel',
    shortcut: '⇧⏎',
    onClick: () => (onSplit ? onSplit() : openSplitPanel(sourceId, label)),
  };
}

/** @param {string} sourceId */
export async function importIntoSource(sourceId) {
  const dir = await pickDirectory();
  if (dir) await importDirectory(dir, { targetSource: sourceId });
}

/**
 * Context menu for a sidebar source or list panel (library view, playlist, USB).
 *
 * @param {object} opts
 * @param {string} opts.sourceId — sidebar key (label, playlist name, or USB path)
 * @param {string} [opts.label] — display name for confirms
 * @param {() => void} [opts.onSplit]
 * @param {() => void} [opts.onNewPlaylist]
 * @param {() => void} [opts.onAddFavorite]
 * @param {(playlist: import('../stores/library.svelte.ts').PlaylistInput) => void} [opts.onRemoveFavorite]
 */
export function buildSourceContextMenuItems(opts) {
  const { sourceId, label = sourceId, onSplit, onNewPlaylist, onAddFavorite, onRemoveFavorite } =
    opts;
  /** @type {MenuItem[]} */
  const items = [];

  if (library.volumes.includes(sourceId)) {
    items.push(
      openSplitItem(sourceId, label, onSplit),
      { isSeparator: true },
      {
        label: 'Sync to USB',
        disabled: busy() || library.playlists.length === 0,
        onClick: async () => {
          await selectVolume(sourceId);
          if (!confirm(`Sync all playlists to ${label}?`)) return;
          await syncToUsb();
        },
      },
      {
        label: 'Wipe USB',
        variant: 'danger',
        disabled: busy(),
        onClick: async () => {
          await selectVolume(sourceId);
          if (
            !confirm(
              `⚠ Wipe all Pioneer library contents from ${label}? This cannot be undone.`
            )
          ) {
            return;
          }
          await wipeVolume(sourceId);
        },
      },
      {
        label: 'Eject',
        disabled: busy(),
        onClick: async () => {
          if (!confirm(`Eject ${label}?`)) return;
          await ejectVolume(sourceId);
        },
      }
    );
    return items;
  }

  if (LIBRARY_VIEWS.has(sourceId)) {
    items.push(openSplitItem(sourceId, label, onSplit));
    if (sourceId === 'All Tracks') {
      items.push(
        {
          label: 'Import folder',
          disabled: busy(),
          onClick: () => importIntoSource(sourceId),
        },
        {
          label: 'Analyze all unanalyzed',
          disabled: busy() || library.tracks.length === 0,
          onClick: () => analyzeTracks(),
        }
      );
    }
    return items;
  }

  const playlist = library.playlists.find((p) => p.name === sourceId);
  if (playlist) {
    const isFavorite = isFavoritePlaylistName(playlist.name);
    items.push(
      openSplitItem(sourceId, label, onSplit),
      {
        label: 'Import folder',
        disabled: busy(),
        onClick: () => importIntoSource(sourceId),
      }
    );

    /** @type {MenuItem[]} */
    const destructive = [];
    if (isFavorite && playlist.track_ids.length > 0) {
      destructive.push({
        label: 'Clear all tracks',
        variant: 'danger',
        onClick: async () => {
          if (!confirm(`Clear all tracks from ${playlist.name}?`)) return;
          await clearPlaylistTracks(playlist.id);
        },
      });
    }
    if (isFavorite) {
      const slot = favoriteSlotFromName(playlist.name);
      if (slot != null && canRemoveFavoriteSlot(slot)) {
        destructive.push({
          label: 'Remove favorite',
          shortcut: '×',
          variant: 'danger',
          onClick: () => onRemoveFavorite?.(playlist),
        });
      }
    }
    if (!isFavorite) {
      destructive.push({
        label: 'Delete playlist',
        variant: 'danger',
        onClick: async () => {
          if (!confirm(`Delete playlist “${playlist.name}”?`)) return;
          await deletePlaylist(playlist.id);
        },
      });
    }
    if (destructive.length > 0) {
      items.push({ isSeparator: true }, ...destructive);
    }
    return items;
  }

  return items;
}

/**
 * Section header menus (sidebar group labels).
 *
 * @param {'favorites'|'library'|'playlists'|'usb'} section
 * @param {object} opts
 * @param {() => void} [opts.onNewPlaylist]
 * @param {() => void} [opts.onAddFavorite]
 * @param {boolean} [opts.canAddFavorite]
 */
export function buildSidebarSectionMenuItems(section, opts = {}) {
  const { onNewPlaylist, onAddFavorite, canAddFavorite = false } = opts;
  /** @type {MenuItem[]} */
  const items = [];
  if (section === 'favorites' && canAddFavorite) {
    items.push({ label: 'Add favorite', onClick: () => onAddFavorite?.() });
  }
  if (section === 'library') {
    items.push(
      {
        label: 'Import folder',
        disabled: busy(),
        onClick: () => importIntoSource('All Tracks'),
      },
      {
        label: 'Analyze all unanalyzed',
        disabled: busy() || library.tracks.length === 0,
        onClick: () => analyzeTracks(),
      }
    );
  }
  if (section === 'playlists') {
    items.push({ label: 'New playlist', onClick: () => onNewPlaylist?.() });
  }
  if (section === 'usb') {
    items.push({
      label: 'Refresh devices',
      disabled: busy(),
      onClick: () => refreshVolumes(),
    });
  }
  return items;
}

/**
 * @param {object} opts
 * @param {number[]} opts.trackIds
 * @param {import('../stores/library.svelte.ts').PlaylistInput | null | undefined} opts.activePlaylist
 * @param {boolean} opts.isUsbView
 * @param {() => void} [opts.onTracksRemoved]
 */
export function buildTrackContextMenuItems(opts) {
  const { trackIds, activePlaylist, isUsbView, onTracksRemoved } = opts;
  if (trackIds.length === 0) return [];

  const favoriteChildren = [];
  for (let slot = 1; slot <= FAVORITE_SLOTS; slot++) {
    const pl = playlistByName(favoritePlaylistName(slot));
    const allIn =
      pl != null && trackIds.length > 0 && trackIds.every((id) => pl.track_ids.includes(id));
    favoriteChildren.push({
      label: `Favorite ${slot}`,
      shortcut: String(slot),
      checked: allIn,
      onClick: () => toggleFavoriteSlot(slot, trackIds),
    });
  }

  const anyAnalyzed = trackIds.some((id) => {
    const t = library.tracks.find((tr) => tr.id === id);
    return t?.analyzed || (t?.raw?.tempo ?? 0) > 0;
  });
  const analyzeLabel =
    trackIds.length > 1
      ? anyAnalyzed
        ? 'Reanalyze tracks'
        : 'Analyze tracks'
      : anyAnalyzed
        ? 'Reanalyze track'
        : 'Analyze track';

  /** @type {MenuItem[]} */
  const items = [
    {
      label: 'Add to Favorites',
      shortcut: '1–9',
      items: favoriteChildren,
    },
  ];

  if (!isUsbView) {
    const playlistItems = library.playlists
      .filter((p) => !isFavoritePlaylistName(p.name))
      .map((pl) => ({
        label: pl.name,
        onClick: () =>
          moveTracksToPlaylist({
            trackIds,
            toPlaylistId: pl.id,
            sourcePlaylistId: activePlaylist?.id ?? null,
            removeFromSource: false,
          }),
      }));

    if (playlistItems.length > 0) {
      items.push({
        label: 'Add to playlist',
        items: playlistItems,
      });
    }

    items.push(
      { isSeparator: true },
      {
        label: analyzeLabel,
        disabled: library.analyzing,
        onClick: () => analyzeTrackIds(trackIds),
      },
      {
        label: 'Set test cues',
        disabled: busy(),
        onClick: () => setTestCuesOnSelected(trackIds),
      },
      { isSeparator: true }
    );

    if (activePlaylist) {
      items.push({
        label:
          trackIds.length > 1
            ? `Remove from ${activePlaylist.name}`
            : 'Remove from playlist',
        onClick: () => removeTracksFromPlaylist(activePlaylist.id, trackIds),
      });
    }

    items.push({
      label: trackIds.length > 1 ? 'Remove from library' : 'Remove from library',
      variant: 'danger',
      onClick: async () => {
        const n = trackIds.length;
        const msg =
          n === 1
            ? 'Remove this track from your library?'
            : `Remove ${n} tracks from your library?`;
        if (!confirm(msg)) return;
        await removeTracks(trackIds);
        onTracksRemoved?.();
      },
    });
  }

  return items;
}

/**
 * @param {object} opts
 * @param {typeof import('../stores/table-columns.svelte.ts').ALL_COLUMN_KEYS} opts.allColumnKeys
 * @param {import('../stores/table-columns.svelte.ts').TableLayout} opts.layout
 * @param {(key: string) => void} opts.toggleColumnVisibility
 * @param {() => void} opts.resetColumns
 * @param {() => void} opts.clearSort
 */
export function buildColumnContextMenuItems(opts) {
  const { allColumnKeys, layout, toggleColumnVisibility, resetColumns, clearSort } =
    opts;

  const columnItems = allColumnKeys.map((meta) => {
    const col = layout.columns.find((c) => c.key === meta.key);
    const hidden =
      col?.hidden ?? (meta.key === 'album' || meta.key === 'genre' || meta.key === 'year');
    return {
      label: meta.label,
      checked: !hidden,
      keepOpen: true,
      onClick: () => toggleColumnVisibility(meta.key),
    };
  });

  return [
    ...columnItems,
    { isSeparator: true },
    { label: 'Reset columns', onClick: () => resetColumns() },
    {
      label: 'Clear sort',
      disabled: !layout.sortKey,
      onClick: () => clearSort(),
    },
  ];
}
