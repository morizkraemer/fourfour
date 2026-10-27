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
  clearAllFavorites,
  deletePlaylist,
  importDirectory,
  syncToUsb,
  wipeVolume,
  ejectVolume,
  revealFile,
  selectVolume,
  refreshVolumes,
  setTestCuesOnSelected,
  FAVORITE_SLOTS,
} from '../stores/library.svelte.ts';
import type { PlaylistInput } from '../services/tauri.svelte.ts';
import type { ALL_COLUMN_KEYS, TableLayout } from '../stores/table-columns.svelte.ts';
import { pickDirectory } from '../services/tauri.svelte.ts';
import { createPlaylistAndOpen } from '../stores/new-playlist.svelte.ts';
import { favouriteTrackIds } from '../lib/newPlaylist.ts';

export function revealLabel(): string {
  const platform = typeof navigator === 'undefined' ? '' : navigator.platform;
  return /mac/i.test(platform) ? 'Show in Finder' : 'Show in folder';
}

export function buildRevealItem(filePath: string | undefined): MenuItem {
  return {
    label: revealLabel(),
    disabled: !filePath,
    onClick: () => revealFile(filePath!),
  };
}

interface MenuItem {
  label?: string;
  shortcut?: string;
  variant?: 'default' | 'danger' | 'disabled';
  disabled?: boolean;
  checked?: boolean;
  keepOpen?: boolean;
  isSeparator?: boolean;
  items?: MenuItem[];
  onClick?: () => void;
}

const LIBRARY_VIEWS = new Set(['All Tracks', 'Recently Added']);

function busy() {
  return library.analyzing || library.syncing;
}

/** Analysis can be queued while a run is already in progress. */
function analyzeBusy() {
  return library.syncing;
}

function hasUnanalyzedTracks() {
  return library.tracks.some((t) => !t.analyzed);
}

export async function importIntoSource(sourceId: string) {
  const dir = await pickDirectory();
  if (dir) await importDirectory(dir, { targetSource: sourceId });
}

function moveFavouritesToNewPlaylistItem(onlySlot?: number): MenuItem {
  const ids = favouriteTrackIds(library.playlists, favoriteSlotFromName, onlySlot);
  return {
    label: 'Move favourite tracks to new playlist',
    disabled: ids.length === 0,
    onClick: () => createPlaylistAndOpen(ids),
  };
}

interface SourceMenuOptions {
  sourceId: string;
  label?: string;
  onNewPlaylist?: () => void;
  onAddFavorite?: () => void;
  onRemoveFavorite?: (playlist: PlaylistInput) => void;
  onRenamePlaylist?: (playlist: PlaylistInput) => void;
}

/** Context menu for a sidebar source or list panel (library view, playlist, USB). */
export function buildSourceContextMenuItems(opts: SourceMenuOptions): MenuItem[] {
  const { sourceId, label = sourceId, onRemoveFavorite, onRenamePlaylist } = opts;
  const items: MenuItem[] = [];

  if (library.volumes.includes(sourceId)) {
    items.push(
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
    if (sourceId === 'All Tracks') {
      items.push(
        {
          label: 'Import folder',
          disabled: busy(),
          onClick: () => importIntoSource(sourceId),
        },
        {
          label: 'Analyze all unanalyzed',
          disabled: analyzeBusy() || library.tracks.length === 0 || !hasUnanalyzedTracks(),
          onClick: () => analyzeTracks(),
        }
      );
    }
    return items;
  }

  const playlist = library.playlists.find((p) => p.name === sourceId);
  if (playlist) {
    const isFavorite = isFavoritePlaylistName(playlist.name);
    if (!isFavorite && onRenamePlaylist) {
      items.push({ label: 'Rename', onClick: () => onRenamePlaylist(playlist) });
    }
    items.push({
      label: 'Import folder',
      disabled: busy(),
      onClick: () => importIntoSource(sourceId),
    });

    if (isFavorite) {
      items.push(moveFavouritesToNewPlaylistItem(favoriteSlotFromName(playlist.name) ?? undefined));
    }

    const destructive: MenuItem[] = [];
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

interface SectionMenuOptions {
  onNewPlaylist?: () => void;
  onAddFavorite?: () => void;
  canAddFavorite?: boolean;
}

/** Section header menus (sidebar group labels). */
export function buildSidebarSectionMenuItems(
  section: 'favorites' | 'library' | 'playlists' | 'usb',
  opts: SectionMenuOptions = {}
): MenuItem[] {
  const { onNewPlaylist, onAddFavorite, canAddFavorite = false } = opts;
  const items: MenuItem[] = [];
  if (section === 'favorites') {
    if (canAddFavorite) items.push({ label: 'Add favorite', onClick: () => onAddFavorite?.() });
    items.push(moveFavouritesToNewPlaylistItem(), {
      label: 'Clear favourites',
      variant: 'danger',
      disabled: favouriteTrackIds(library.playlists, favoriteSlotFromName).length === 0,
      onClick: async () => {
        if (!confirm('Clear all favourite tracks? The tracks stay in your library.')) return;
        await clearAllFavorites();
      },
    });
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
        disabled: analyzeBusy() || library.tracks.length === 0 || !hasUnanalyzedTracks(),
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

interface TrackMenuOptions {
  trackIds: number[];
  activePlaylist: PlaylistInput | null | undefined;
  isUsbView: boolean;
  onTracksRemoved?: () => void;
}

export function buildTrackContextMenuItems(opts: TrackMenuOptions): MenuItem[] {
  const { trackIds, activePlaylist, isUsbView, onTracksRemoved } = opts;
  if (trackIds.length === 0) return [];

  const favoriteChildren: MenuItem[] = [];
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

  const items: MenuItem[] = [
    {
      label: 'Add to Favorites',
      shortcut: '1–9',
      items: favoriteChildren,
    },
  ];

  const revealItems: MenuItem[] =
    trackIds.length === 1
      ? [buildRevealItem(library.tracks.find((t) => t.id === trackIds[0])?.filePath)]
      : [];

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
      ...revealItems,
      { isSeparator: true },
      {
        label: analyzeLabel,
        disabled: analyzeBusy(),
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
  } else {
    items.push(...revealItems);
  }

  return items;
}

interface ColumnMenuOptions {
  allColumnKeys: typeof ALL_COLUMN_KEYS;
  layout: TableLayout;
  toggleColumnVisibility: (key: string) => void;
  resetColumns: () => void;
  clearSort: () => void;
}

export function buildColumnContextMenuItems(opts: ColumnMenuOptions): MenuItem[] {
  const { allColumnKeys, layout, toggleColumnVisibility, resetColumns, clearSort } =
    opts;

  const columnItems: MenuItem[] = allColumnKeys.map((meta) => {
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
