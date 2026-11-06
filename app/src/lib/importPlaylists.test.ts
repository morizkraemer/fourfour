import { describe, expect, it } from 'vitest';
import { dropPlan, folderPlan, pendingNames, plannedFileCount, trackIdsForFiles } from './importPlaylists.ts';

describe('folderPlan', () => {
  it('turns a flat folder into one playlist named after it', () => {
    expect(folderPlan('/Music/Fresh Finds', ['/Music/Fresh Finds/a.mp3', '/Music/Fresh Finds/b.flac'])).toEqual({
      kind: 'playlist',
      name: 'Fresh Finds',
      files: ['/Music/Fresh Finds/a.mp3', '/Music/Fresh Finds/b.flac'],
    });
  });

  it('mirrors subfolders as a playlist folder, with a playlist for the files directly inside', () => {
    const plan = folderPlan('/Music/Set/', [
      '/Music/Set/Peak/Track 10.mp3',
      '/Music/Set/intro.mp3',
      '/Music/Set/Openers/Deep/one.mp3',
      '/Music/Set/Openers/two.mp3',
      '/Music/Set/Peak/Track 2.mp3',
    ]);
    expect(plan).toEqual({
      kind: 'folder',
      name: 'Set',
      children: [
        { kind: 'playlist', name: 'Set', files: ['/Music/Set/intro.mp3'] },
        {
          kind: 'folder',
          name: 'Openers',
          children: [
            { kind: 'playlist', name: 'Openers', files: ['/Music/Set/Openers/two.mp3'] },
            { kind: 'playlist', name: 'Deep', files: ['/Music/Set/Openers/Deep/one.mp3'] },
          ],
        },
        { kind: 'playlist', name: 'Peak', files: ['/Music/Set/Peak/Track 10.mp3', '/Music/Set/Peak/Track 2.mp3'] },
      ],
    });
  });

  it('leaves out a folder without audio and ignores files outside the root', () => {
    expect(folderPlan('C:\\Music\\Empty', ['C:\\Music\\Other\\x.mp3'])).toBeNull();
  });
});

describe('dropPlan', () => {
  it('plans each dropped folder and gathers loose files into one playlist', () => {
    const plans = dropPlan(
      [
        { path: '/a.mp3', folder: false, files: ['/a.mp3'] },
        { path: '/Crate', folder: true, files: ['/Crate/x.mp3'] },
        { path: '/Nothing', folder: true, files: [] },
        { path: '/b.mp3', folder: false, files: ['/b.mp3'] },
      ],
      'New playlist',
    );
    expect(plans).toEqual([
      { kind: 'playlist', name: 'Crate', files: ['/Crate/x.mp3'] },
      { kind: 'playlist', name: 'New playlist', files: ['/a.mp3', '/b.mp3'] },
    ]);
    expect(plannedFileCount(plans)).toBe(3);
  });
});

describe('pendingNames', () => {
  it('shows file names without extension in natural order', () => {
    expect(pendingNames(['/x/Track 10.mp3', '/x/Track 2.flac', '/x/intro'])).toEqual(['intro', 'Track 2', 'Track 10']);
  });
});

describe('trackIdsForFiles', () => {
  it('finds library ids by path, including skipped duplicates, in library order', () => {
    const idsFor = trackIdsForFiles(
      [
        { id: 5, filePath: '/Set/b.mp3' },
        { id: 3, filePath: '/Old/dupe.mp3' },
        { id: 9, filePath: '/Set/a.mp3' },
      ],
      [{ path: '/Set/dupe.mp3', id: 3 }],
    );
    expect(idsFor(['/Set/a.mp3', '/Set/dupe.mp3', '/Set/b.mp3', '/Set/missing.mp3'])).toEqual([5, 3, 9]);
  });
});
