import { describe, expect, it } from 'vitest';
import {
  buildColumnContextMenuItems,
  buildFolderContextMenuItems,
  buildSidebarSectionMenuItems,
  buildSourceContextMenuItems,
  buildTrackContextMenuItems,
  buildRevealItem,
  revealLabel,
} from './context-menus.ts';
import { ALL_COLUMN_KEYS, getLayout } from '../stores/table-columns.svelte.ts';
import { FAVORITE_SLOTS, library } from '../stores/library.svelte.ts';

describe('buildSidebarSectionMenuItems', () => {
  it('offers Add favorite only when allowed', () => {
    expect(buildSidebarSectionMenuItems('favorites').map((i) => i.label)).not.toContain('Add favorite');
    const items = buildSidebarSectionMenuItems('favorites', { canAddFavorite: true });
    expect(items.map((i) => i.label)).toContain('Add favorite');
  });
  it('calls the New playlist callback', () => {
    let called = 0;
    const [item] = buildSidebarSectionMenuItems('playlists', { onNewPlaylist: () => called++ });
    expect(item.label).toBe('New playlist');
    item.onClick!();
    expect(called).toBe(1);
  });
  it('disables Analyze all when the library is empty', () => {
    const items = buildSidebarSectionMenuItems('library');
    expect(items.map((i) => i.label)).toEqual(['Import folder', 'Analyze all unanalyzed']);
    expect(library.tracks).toHaveLength(0);
    expect(items[1].disabled).toBe(true);
  });
  it('has a Refresh devices entry for usb', () => {
    expect(buildSidebarSectionMenuItems('usb').map((i) => i.label)).toEqual(['Refresh devices']);
  });
});

describe('buildTrackContextMenuItems', () => {
  it('returns nothing for an empty selection', () => {
    expect(buildTrackContextMenuItems({ trackIds: [], activePlaylist: null, isUsbView: false })).toEqual([]);
  });
  it('only offers favorites and reveal on a USB view', () => {
    const items = buildTrackContextMenuItems({ trackIds: [1], activePlaylist: null, isUsbView: true });
    expect(items.map((i) => i.label)).toEqual(['Add to Favorites', revealLabel()]);
    expect(items[0].items).toHaveLength(FAVORITE_SLOTS);
    expect(items[0].items![0]).toMatchObject({ label: 'Favorite 1', shortcut: '1', checked: false });
  });
  it('offers reveal for a single selection only', () => {
    const one = buildTrackContextMenuItems({ trackIds: [1], activePlaylist: null, isUsbView: false });
    const many = buildTrackContextMenuItems({ trackIds: [1, 2], activePlaylist: null, isUsbView: false });
    expect(one.map((i) => i.label)).toContain(revealLabel());
    expect(many.map((i) => i.label)).not.toContain(revealLabel());
  });
  it('disables reveal for a track without a file path', () => {
    expect(buildRevealItem(undefined).disabled).toBe(true);
    expect(buildRevealItem('/a/b.mp3').disabled).toBe(false);
  });
  it('uses singular and plural labels and a danger remove entry', () => {
    const one = buildTrackContextMenuItems({ trackIds: [1], activePlaylist: null, isUsbView: false });
    const many = buildTrackContextMenuItems({ trackIds: [1, 2], activePlaylist: null, isUsbView: false });
    expect(one.map((i) => i.label)).toContain('Analyze track');
    expect(many.map((i) => i.label)).toContain('Analyze tracks');
    expect(one.at(-1)).toMatchObject({ label: 'Remove from library', variant: 'danger' });
  });
  it('names the active playlist in the remove entry for multi-selection', () => {
    const pl = { id: 5, name: 'Warmup', track_ids: [1, 2], parent_id: 0, is_folder: false } as any;
    const many = buildTrackContextMenuItems({ trackIds: [1, 2], activePlaylist: pl, isUsbView: false });
    const single = buildTrackContextMenuItems({ trackIds: [1], activePlaylist: pl, isUsbView: false });
    expect(many.map((i) => i.label)).toContain('Remove from Warmup');
    expect(single.map((i) => i.label)).toContain('Remove from playlist');
  });
});

describe('playlist export entries', () => {
  const peak = { id: 1, name: 'Peak Time', track_ids: [1, 2], parent_id: 0, is_folder: false };
  const sets = { id: 9, name: 'Sets', track_ids: [], parent_id: 0, is_folder: true };

  it('lists Export playlist with all five formats, and Copy tracklist, on a playlist', () => {
    library.playlists = [peak];
    const items = buildSourceContextMenuItems({ sourceId: 'Peak Time' });
    const exportEntry = items.find((i) => i.label === 'Export playlist…')!;
    expect(exportEntry.items!.map((i) => i.label)).toEqual([
      'M3U8 playlist',
      'CSV spreadsheet',
      'Text tracklist',
      'Traktor (NML)',
      'Rekordbox XML',
    ]);
    expect(items.map((i) => i.label)).toContain('Copy tracklist');
    library.playlists = [];
  });

  it('exports a whole folder only as Traktor or Rekordbox', () => {
    const items = buildFolderContextMenuItems({ folder: sets });
    const exportEntry = items.find((i) => i.label === 'Export folder…')!;
    expect(exportEntry.items!.map((i) => i.label)).toEqual(['Traktor (NML)', 'Rekordbox XML']);
  });
});

describe('revealLabel', () => {
  it('uses platform wording', () => {
    const original = Object.getOwnPropertyDescriptor(globalThis, 'navigator');
    const set = (platform: string) =>
      Object.defineProperty(globalThis, 'navigator', { value: { platform }, configurable: true });
    set('MacIntel');
    expect(revealLabel()).toBe('Show in Finder');
    set('Linux x86_64');
    expect(revealLabel()).toBe('Show in folder');
    if (original) Object.defineProperty(globalThis, 'navigator', original);
    else delete (globalThis as any).navigator;
  });
});

describe('buildColumnContextMenuItems', () => {
  const keys = [
    { key: 'title', label: 'Title' },
    { key: 'album', label: 'Album' },
    { key: 'bpm', label: 'BPM' },
  ];
  const noop = () => {};
  const layout = (over: Record<string, unknown> = {}) =>
    ({
      columns: [{ key: 'title' }, { key: 'bpm', hidden: true }],
      sortKey: null,
      sortDir: 'asc',
      ...over,
    }) as any;

  it('derives checked state from layout, treating columns missing from it as hidden', () => {
    const items = buildColumnContextMenuItems({
      allColumnKeys: keys,
      layout: layout(),
      toggleColumnVisibility: noop,
      resetColumns: noop,
      clearSort: noop,
    });
    expect(items.slice(0, 3).map((i) => i.checked)).toEqual([true, false, false]);
  });
  it('lists every catalog column including Rating and Colour, unchecked on a fresh layout', () => {
    const items = buildColumnContextMenuItems({
      allColumnKeys: ALL_COLUMN_KEYS,
      layout: getLayout('library'),
      toggleColumnVisibility: noop,
      resetColumns: noop,
      clearSort: noop,
    });
    const byLabel = new Map(items.map((i) => [i.label, i.checked]));
    expect(byLabel.get('Rating')).toBe(false);
    expect(byLabel.get('Colour')).toBe(false);
    expect(byLabel.get('Title')).toBe(true);
    expect(items.slice(0, ALL_COLUMN_KEYS.length).map((i) => i.label)).toEqual(
      ALL_COLUMN_KEYS.map((c) => c.label)
    );
  });
  it('disables Clear sort without a sort key and wires toggle callbacks', () => {
    const toggled: string[] = [];
    const base = {
      allColumnKeys: keys,
      toggleColumnVisibility: (k: string) => toggled.push(k),
      resetColumns: noop,
      clearSort: noop,
    };
    const unsorted = buildColumnContextMenuItems({ ...base, layout: layout() });
    const sorted = buildColumnContextMenuItems({ ...base, layout: layout({ sortKey: 'bpm' }) });
    expect(unsorted.at(-1)).toMatchObject({ label: 'Clear sort', disabled: true });
    expect(sorted.at(-1)!.disabled).toBe(false);
    unsorted[0].onClick!();
    expect(toggled).toEqual(['title']);
    expect(unsorted[3].isSeparator).toBe(true);
  });
});

describe('favourites to playlist', () => {
  it('offers a move item and a separate clear item, both disabled when empty', () => {
    const items = buildSidebarSectionMenuItems('favorites');
    expect(items.map((i) => i.label)).toEqual(['Move favourite tracks to new playlist', 'Clear favourites']);
    expect(items.every((i) => i.disabled)).toBe(true);
  });
  it('lists the move item first and never clears on its own', () => {
    const [move, clear] = buildSidebarSectionMenuItems('favorites');
    expect(move.variant).toBeUndefined();
    expect(clear.variant).toBe('danger');
  });
});

describe('buildSourceContextMenuItems rename', () => {
  it('offers Rename first for a normal playlist and not for a favorite slot', () => {
    library.playlists = [
      { id: 1, name: 'Warmup', track_ids: [], parent_id: 0, is_folder: false },
      { id: 2, name: 'Favorites 1', track_ids: [], parent_id: 0, is_folder: false },
    ];
    const renamed: string[] = [];
    const onRenamePlaylist = (p: { name: string }) => renamed.push(p.name);
    const normal = buildSourceContextMenuItems({ sourceId: 'Warmup', onRenamePlaylist });
    expect(normal[0].label).toBe('Rename');
    normal[0].onClick!();
    expect(renamed).toEqual(['Warmup']);
    const favorite = buildSourceContextMenuItems({ sourceId: 'Favorites 1', onRenamePlaylist });
    expect(favorite.map((i) => i.label)).not.toContain('Rename');
    expect(buildSourceContextMenuItems({ sourceId: 'Warmup' }).map((i) => i.label)).not.toContain('Rename');
    library.playlists = [];
  });
});

describe('playlist folder menus', () => {
  const tree = () => [
    { id: 1, name: 'Sets', track_ids: [], parent_id: 0, is_folder: true },
    { id: 2, name: 'Friday', track_ids: [5], parent_id: 1, is_folder: false },
    { id: 3, name: 'Loose', track_ids: [], parent_id: 0, is_folder: false },
  ];

  it('offers new playlist, new folder and import folder on the playlists section', () => {
    expect(buildSidebarSectionMenuItems('playlists').map((i) => i.label)).toEqual([
      'New playlist',
      'New folder',
      undefined,
      'Import folder',
    ]);
  });

  it('gives a playlist rename, duplicate, creation, import, stick, export and delete entries', () => {
    library.playlists = tree();
    const labels = buildSourceContextMenuItems({ sourceId: 'Friday', onRenamePlaylist: () => {} }).map((i) => i.label);
    expect(labels).toEqual([
      'Rename',
      'Duplicate',
      undefined,
      'New playlist',
      'New folder',
      undefined,
      'Import folder',
      'Put on stick',
      undefined,
      'Export playlist…',
      'Copy tracklist',
      undefined,
      'Delete playlist',
    ]);
    library.playlists = [];
  });

  it('lists the connected sticks under Put on stick and disables it with none', () => {
    library.playlists = tree();
    library.volumes = ['/Volumes/STICK A', '/Volumes/STICK B'];
    const find = () => buildSourceContextMenuItems({ sourceId: 'Loose' }).find((i) => i.label === 'Put on stick')!;
    expect(find().items!.map((i) => i.label)).toEqual(['STICK A', 'STICK B']);
    library.volumes = [];
    expect(find().disabled).toBe(true);
    library.playlists = [];
  });

  it('gives a folder creation, rename and delete entries', () => {
    library.playlists = tree();
    const renamed: string[] = [];
    const items = buildFolderContextMenuItems({ folder: library.playlists[0], onRename: (f) => renamed.push(f.name) });
    expect(items.map((i) => i.label)).toEqual([
      'New playlist',
      'New folder',
      undefined,
      'Rename',
      undefined,
      'Export folder…',
      undefined,
      'Delete folder',
    ]);
    items.find((i) => i.label === 'Rename')!.onClick!();
    expect(renamed).toEqual(['Sets']);
    expect(items.at(-1)!.variant).toBe('danger');
    library.playlists = [];
  });

  it('does not treat a folder as the playlist of the same name', () => {
    library.playlists = [{ id: 1, name: 'Sets', track_ids: [], parent_id: 0, is_folder: true }];
    expect(buildSourceContextMenuItems({ sourceId: 'Sets' })).toEqual([]);
    library.playlists = [];
  });
});
