import { describe, expect, it } from 'vitest';
import { dropPointToCss, droppedFolderName, fileDropTargetFor, trackIdsUnderPaths } from './fileDrop.ts';

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

  it('prefers the new playlist zone when both attributes are present', () => {
    expect(
      fileDropTargetFor(hitOn({ 'data-new-playlist-drop': '', 'data-drop-track-playlist-id': '3' }))
    ).toEqual({ kind: 'new-playlist' });
  });
});

describe('dropPointToCss', () => {
  it('divides physical pixels by the device pixel ratio', () => {
    expect(dropPointToCss({ x: 400, y: 200 }, 2)).toEqual({ x: 200, y: 100 });
    expect(dropPointToCss({ x: 400, y: 200 }, 0)).toEqual({ x: 400, y: 200 });
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

describe('droppedFolderName', () => {
  it('names the playlist after the dropped folder', () => {
    expect(droppedFolderName(tracks, ['/Music/Set One'])).toBe('Set One');
    expect(droppedFolderName(tracks, ['/Music/Set One/'])).toBe('Set One');
  });
  it('is null when only files were dropped', () => {
    expect(droppedFolderName(tracks, ['/Music/single.mp3'])).toBeNull();
  });
});
