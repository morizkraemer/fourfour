import { describe, expect, it, vi } from 'vitest';
import { buildPaletteGroups, isCommandQuery, type PaletteSources } from './palette-model.ts';

function sources(run = vi.fn()): PaletteSources {
  return {
    actions: [
      { id: 'play', label: 'Play / pause', shortcut: 'Space', run },
      { id: 'new', label: 'New playlist', shortcut: '⌘⇧N', run },
      { id: 'settings', label: 'Open settings', shortcut: '⌘,', run },
    ],
    settings: [
      {
        id: 'analysis.autoAnalyzeNew',
        label: 'Settings: Analyze new tracks',
        control: { kind: 'switch', on: true },
        change: run,
      },
      { id: 'page.audio', label: 'Open settings › Audio', change: run },
    ],
    places: [
      { id: 'all', label: 'All Tracks', icon: 'list', run },
      { id: 'p1', label: 'Peak Time', sub: 'playlist', icon: 'list', run },
    ],
    tracks: [
      { id: 1, title: 'Hyperion', artist: 'Solomun', run },
      { id: 2, title: 'Voidwalker', artist: 'Tale Of Us', run },
    ],
  };
}

const ids = (g: ReturnType<typeof buildPaletteGroups>, key: string) =>
  g.find((x) => x.key === key)!.items.map((i) => i.id);

describe('buildPaletteGroups', () => {
  it('shows actions and places, but no tracks, for an empty query', () => {
    const g = buildPaletteGroups('', sources());
    expect(ids(g, 'actions')).toHaveLength(3);
    expect(ids(g, 'places')).toHaveLength(2);
    expect(ids(g, 'tracks')).toEqual([]);
  });

  it('carries the shortcut text on action rows', () => {
    const g = buildPaletteGroups('play', sources());
    expect(g.find((x) => x.key === 'actions')!.items.find((i) => i.id === 'action:play')?.right).toBe('Space');
  });

  it('finds playlists by fuzzy query', () => {
    expect(ids(buildPaletteGroups('peak', sources()), 'places')).toEqual(['place:p1']);
  });

  it('lists the tracks the host matched, with the artist as sub text', () => {
    const g = buildPaletteGroups('solo', sources());
    const tracks = g.find((x) => x.key === 'tracks')!.items;
    expect(tracks.map((t) => t.id)).toEqual(['track:1', 'track:2']);
    expect(tracks[0].sub).toBe('Solomun');
  });

  it('highlights only the title', () => {
    const item = buildPaletteGroups('tale', sources()).find((x) => x.key === 'tracks')!.items[1];
    expect(item.ranges ?? []).toEqual([]);
  });

  it('caps tracks at the query limit', () => {
    const src = sources();
    src.tracks = Array.from({ length: 20 }, (_, i) => ({ id: i, title: `Song ${i}`, artist: 'A', run: vi.fn() }));
    expect(ids(buildPaletteGroups('song', src), 'tracks')).toHaveLength(8);
  });

  it('orders groups tracks, go to, commands, settings', () => {
    const g = buildPaletteGroups('s', sources());
    expect(g.map((x) => x.key)).toEqual(['tracks', 'places', 'actions', 'settings']);
  });

  it('a > prefix keeps only commands and settings', () => {
    const g = buildPaletteGroups('>play', sources());
    expect(ids(g, 'actions')[0]).toBe('action:play');
    expect(ids(g, 'tracks')).toEqual([]);
    expect(ids(g, 'places')).toEqual([]);
  });

  it('a bare > lists every command and no settings or tracks', () => {
    const g = buildPaletteGroups('>', sources());
    expect(ids(g, 'actions')).toHaveLength(3);
    expect(ids(g, 'settings')).toEqual([]);
    expect(ids(g, 'tracks')).toEqual([]);
  });

  it('a > query still reaches settings rows', () => {
    expect(ids(buildPaletteGroups('> analyze new', sources()), 'settings')).toEqual([
      'setting:analysis.autoAnalyzeNew',
    ]);
  });

  it('detects the command prefix', () => {
    expect(isCommandQuery('>x')).toBe(true);
    expect(isCommandQuery('  > x')).toBe(true);
    expect(isCommandQuery('x>')).toBe(false);
  });

  it('runs the matching entry', () => {
    const run = vi.fn();
    buildPaletteGroups('new', sources(run)).find((g) => g.key === 'actions')!.items[0].run();
    expect(run).toHaveBeenCalledTimes(1);
  });

  it('hides settings rows until the query asks for them', () => {
    expect(ids(buildPaletteGroups('', sources()), 'settings')).toEqual([]);
    expect(ids(buildPaletteGroups('analyze new', sources()), 'settings')).toEqual([
      'setting:analysis.autoAnalyzeNew',
    ]);
  });

  it('keeps the palette open on control rows and routes the chosen value', () => {
    const change = vi.fn();
    const src = sources();
    src.settings = [
      {
        id: 'e',
        label: 'Settings: Analysis engine',
        control: { kind: 'choice', value: 'auto', options: [{ value: 'auto', label: 'Automatic' }] },
        change,
      },
      { id: 'page.audio', label: 'Open settings › Audio', change },
    ];
    const rows = buildPaletteGroups('settings', src).find((g) => g.key === 'settings')!.items;
    const engine = rows.find((r) => r.id === 'setting:e')!;
    const page = rows.find((r) => r.id === 'setting:page.audio')!;
    expect(engine.keepOpen).toBe(true);
    expect(page.keepOpen).toBe(false);
    engine.choose!('python');
    expect(change).toHaveBeenCalledWith('python');
    engine.run();
    expect(change).toHaveBeenLastCalledWith();
  });
});
