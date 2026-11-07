import { describe, expect, it } from 'vitest';
import { dropGroups, dropPointToCss, dropPositionScale, fileDropTargetFor, trackIdsUnderPaths } from './fileDrop.ts';

function hitOn(attrs: Record<string, string>) {
  return {
    closest: (selector: string) => {
      const name = selector.replace(/^\[|\]$/g, '');
      return name in attrs ? { getAttribute: (n: string) => attrs[n] ?? null } : null;
    },
  };
}

describe('fileDropTargetFor', () => {
  it('targets the new playlist zone, the sidebar playlist under the pointer, or the open view', () => {
    expect(fileDropTargetFor(hitOn({ 'data-new-playlist-drop': '' }))).toEqual({ kind: 'new-playlist' });
    expect(fileDropTargetFor(hitOn({ 'data-drop-track-playlist-id': '7' }))).toEqual({
      kind: 'playlist',
      playlistId: 7,
    });
    expect(fileDropTargetFor(hitOn({}))).toEqual({ kind: 'view' });
    expect(fileDropTargetFor(null)).toEqual({ kind: 'view' });
  });

  it('targets a playlist folder by its id', () => {
    expect(fileDropTargetFor(hitOn({ 'data-playlist-folder': '', 'data-playlist-id': '5' }))).toEqual({
      kind: 'folder',
      folderId: 5,
    });
  });

  it('prefers the new playlist zone when both attributes are present', () => {
    expect(
      fileDropTargetFor(hitOn({ 'data-new-playlist-drop': '', 'data-drop-track-playlist-id': '3' }))
    ).toEqual({ kind: 'new-playlist' });
  });
});

describe('dropPointToCss', () => {
  it('scales Windows physical pixels to CSS pixels and leaves macOS and Linux points alone', () => {
    const mac = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15';
    const win = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Edg/120.0';
    expect(dropPointToCss({ x: 400, y: 200 }, dropPositionScale(mac, 2))).toEqual({ x: 400, y: 200 });
    expect(dropPointToCss({ x: 400, y: 200 }, dropPositionScale(win, 2))).toEqual({ x: 200, y: 100 });
    expect(dropPositionScale(win, 0)).toBe(1);
  });
});

const tracks = [
  { id: 1, filePath: '/Music/Set One/a.mp3' },
  { id: 2, filePath: '/Music/Set One/deep/b.mp3' },
  { id: 3, filePath: '/Music/Set One Extra/c.mp3' },
  { id: 4, filePath: '/Music/single.mp3' },
];

describe('trackIdsUnderPaths', () => {
  it('includes tracks already in the library that sit inside a dropped folder', () => {
    expect(trackIdsUnderPaths(tracks, ['/Music/Set One'])).toEqual([1, 2]);
  });
  it('matches dropped files exactly and tolerates trailing slashes', () => {
    expect(trackIdsUnderPaths(tracks, ['/Music/single.mp3', '/Music/Set One/'])).toEqual([1, 2, 4]);
  });
  it('handles windows separators', () => {
    expect(trackIdsUnderPaths([{ id: 9, filePath: 'C:\\Music\\Mix\\x.mp3' }], ['C:\\Music\\Mix'])).toEqual([9]);
  });
});

describe('trackIdsUnderPaths against the walk it replaced', () => {
  it('gives the same ids in the same order on random libraries and dropped paths', () => {
    let state = 777;
    const random = () => {
      state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
      return state / 0x100000000;
    };
    const folders = ['/Music/Set', '/Music/Set/deep', 'C:\\Music\\Mix', '/Music/Set Extra', '/Downloads/Crate', '/Music/Set/'];
    const pathOf = () => {
      const folder = folders[Math.floor(random() * folders.length)].replace(/[\\/]+$/, '');
      const slash = folder.includes('\\') ? '\\' : '/';
      return `${folder}${slash}${Math.floor(random() * 40)}.mp3${random() < 0.05 ? slash : ''}`;
    };
    const reference = (rows: { id: number; filePath?: string }[], dropped: string[]) => {
      const normalize = (path: string) => path.replace(/\\/g, '/').replace(/\/+$/, '');
      const files = new Set(dropped.map(normalize));
      return rows
        .filter((t) => t.filePath && (files.has(normalize(t.filePath)) || dropped.some((p) => normalize(t.filePath!).startsWith(`${normalize(p)}/`))))
        .map((t) => t.id);
    };
    for (let round = 0; round < 400; round++) {
      const rows = Array.from({ length: Math.floor(random() * 80) }, () => ({ id: Math.floor(random() * 1000), filePath: random() < 0.05 ? undefined : pathOf() }));
      const dropped = Array.from({ length: 1 + Math.floor(random() * 3) }, () => (random() < 0.5 ? folders[Math.floor(random() * folders.length)] : pathOf()));
      expect(trackIdsUnderPaths(rows, dropped)).toEqual(reference(rows, dropped));
    }
  });
});

describe('dropGroups', () => {
  it('makes one group per dropped folder, named after it', () => {
    expect(dropGroups(tracks, ['/Music/Set One/', '/Music/Set One Extra'])).toEqual([
      { folder: 'Set One', ids: [1, 2] },
      { folder: 'Set One Extra', ids: [3] },
    ]);
  });
  it('puts loose files in one unnamed group and adds skipped duplicates where they were dropped', () => {
    const matches = [{ path: '/Downloads/dupe.mp3', id: 3 }, { path: '/Music/Set One/copy.mp3', id: 4 }];
    expect(dropGroups(tracks, ['/Music/single.mp3', '/Downloads/dupe.mp3', '/Music/Set One'], matches)).toEqual([
      { folder: 'Set One', ids: [1, 2, 4] },
      { folder: null, ids: [4, 3] },
    ]);
  });
  it('is empty when nothing was found', () => {
    expect(dropGroups(tracks, ['/Elsewhere'])).toEqual([]);
  });
});
