import { describe, expect, it } from 'vitest';
import {
  buildColumnContextMenuItems,
  buildSidebarSectionMenuItems,
  buildTrackContextMenuItems,
} from './context-menus.ts';
import { FAVORITE_SLOTS, library } from '../stores/library.svelte.ts';

describe('buildSidebarSectionMenuItems', () => {
  it('offers Add favorite only when allowed', () => {
    expect(buildSidebarSectionMenuItems('favorites')).toEqual([]);
    const items = buildSidebarSectionMenuItems('favorites', { canAddFavorite: true });
    expect(items.map((i) => i.label)).toEqual(['Add favorite']);
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
  it('only offers favorites on a USB view', () => {
    const items = buildTrackContextMenuItems({ trackIds: [1], activePlaylist: null, isUsbView: true });
    expect(items.map((i) => i.label)).toEqual(['Add to Favorites']);
    expect(items[0].items).toHaveLength(FAVORITE_SLOTS);
    expect(items[0].items![0]).toMatchObject({ label: 'Favorite 1', shortcut: '1', checked: false });
  });
  it('uses singular and plural labels and a danger remove entry', () => {
    const one = buildTrackContextMenuItems({ trackIds: [1], activePlaylist: null, isUsbView: false });
    const many = buildTrackContextMenuItems({ trackIds: [1, 2], activePlaylist: null, isUsbView: false });
    expect(one.map((i) => i.label)).toContain('Analyze track');
    expect(many.map((i) => i.label)).toContain('Analyze tracks');
    expect(one.at(-1)).toMatchObject({ label: 'Remove from library', variant: 'danger' });
  });
  it('names the active playlist in the remove entry for multi-selection', () => {
    const pl = { id: 5, name: 'Warmup', track_ids: [1, 2] } as any;
    const many = buildTrackContextMenuItems({ trackIds: [1, 2], activePlaylist: pl, isUsbView: false });
    const single = buildTrackContextMenuItems({ trackIds: [1], activePlaylist: pl, isUsbView: false });
    expect(many.map((i) => i.label)).toContain('Remove from Warmup');
    expect(single.map((i) => i.label)).toContain('Remove from playlist');
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
    ({ columns: [{ key: 'bpm', hidden: true }], sortKey: null, sortDir: 'asc', ...over }) as any;

  it('derives checked state from layout, defaulting album/genre/year hidden', () => {
    const items = buildColumnContextMenuItems({
      allColumnKeys: keys,
      layout: layout(),
      toggleColumnVisibility: noop,
      resetColumns: noop,
      clearSort: noop,
    });
    expect(items.slice(0, 3).map((i) => i.checked)).toEqual([true, false, false]);
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
