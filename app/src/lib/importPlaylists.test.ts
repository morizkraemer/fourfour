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

function referenceIdsForFiles(
  tracks: ReadonlyArray<{ id: number; filePath?: string }>,
  matches: ReadonlyArray<{ path: string; id: number }>,
): (files: readonly string[]) => number[] {
  const normalize = (path: string) => path.replace(/\\/g, '/').replace(/\/+$/, '');
  const order = new Map<number, number>();
  const byPath = new Map<string, number>();
  tracks.forEach((track, index) => {
    order.set(track.id, index);
    if (track.filePath) byPath.set(normalize(track.filePath), track.id);
  });
  for (const match of matches) byPath.set(normalize(match.path), match.id);
  return (files) => {
    const ids = new Set<number>();
    for (const file of files) {
      const id = byPath.get(normalize(file));
      if (id !== undefined) ids.add(id);
    }
    return [...ids].sort((a, b) => (order.get(a) ?? Infinity) - (order.get(b) ?? Infinity));
  };
}

describe('trackIdsForFiles', () => {
  it('finds library ids by path, including skipped duplicates, in library order', () => {
    const files = ['/Set/a.mp3', '/Set/dupe.mp3', '/Set/b.mp3', '/Set/missing.mp3'];
    const idsFor = trackIdsForFiles(
      [
        { id: 5, filePath: '/Set/b.mp3' },
        { id: 3, filePath: '/Old/dupe.mp3' },
        { id: 9, filePath: '/Set/a.mp3' },
      ],
      [{ path: '/Set/dupe.mp3', id: 3 }],
      files,
    );
    expect(idsFor(files)).toEqual([5, 3, 9]);
  });

  it('gives the ids and the order the walk over every row gave, on random libraries', () => {
    let state = 12345;
    const random = () => {
      state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
      return state / 0x100000000;
    };
    const names = ['a', 'B', 'c d', 'Track 1', 'x'];
    const pathOf = () => {
      const folder = ['/Music/Set', '/Music/Set/deep', 'C:\\Music\\Mix', '/Downloads/Crate/F1', '/other'][Math.floor(random() * 5)];
      const file = `${names[Math.floor(random() * names.length)]}${Math.floor(random() * 30)}.mp3`;
      const slash = folder.includes('\\') ? '\\' : '/';
      const trailing = random() < 0.1 ? slash : '';
      return `${folder}${slash}${file}${trailing}`;
    };
    for (let round = 0; round < 300; round++) {
      const count = Math.floor(random() * 60);
      const ids = Array.from({ length: count }, (_, i) => i + 1);
      for (let i = ids.length - 1; i > 0; i--) {
        const j = Math.floor(random() * (i + 1));
        [ids[i], ids[j]] = [ids[j], ids[i]];
      }
      const tracks = ids.map((id) => ({ id, filePath: random() < 0.05 ? undefined : pathOf() }));
      const asked = Array.from({ length: Math.floor(random() * 40) }, () => (random() < 0.5 && tracks.length > 0 ? tracks[Math.floor(random() * tracks.length)].filePath ?? pathOf() : pathOf()));
      const matches = Array.from({ length: Math.floor(random() * 6) }, () => ({
        path: random() < 0.6 && asked.length > 0 ? asked[Math.floor(random() * asked.length)] : pathOf(),
        id: random() < 0.15 ? 1000 + Math.floor(random() * 5) : 1 + Math.floor(random() * Math.max(1, count)),
      }));
      const expected = referenceIdsForFiles(tracks, matches);
      const actual = trackIdsForFiles(tracks, matches, asked);
      for (let slice = 0; slice < 4; slice++) {
        const files = asked.filter(() => random() < 0.5);
        expect(actual(files)).toEqual(expected(files));
      }
      expect(actual(asked)).toEqual(expected(asked));
    }
  });
});
