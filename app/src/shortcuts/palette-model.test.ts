import { describe, expect, it, vi } from 'vitest';
import { buildPaletteGroups, type PaletteSources } from './palette-model.ts';

function sources(run = vi.fn()): PaletteSources {
  return {
    actions: [
      { id: 'play', label: 'Play / pause', shortcut: 'Space', run },
      { id: 'new', label: 'New playlist', shortcut: '⌘⇧N', run },
      { id: 'settings', label: 'Open settings', shortcut: '⌘,', run },
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
    expect(g[0].items.find((i) => i.id === 'action:play')?.right).toBe('Space');
  });

  it('finds playlists and tracks by fuzzy query', () => {
    const peak = buildPaletteGroups('peak', sources());
    expect(ids(peak, 'places')).toEqual(['place:p1']);
    const track = buildPaletteGroups('solo', sources());
    expect(ids(track, 'tracks')).toEqual(['track:1']);
    expect(track.find((g) => g.key === 'tracks')!.items[0].sub).toBe('Solomun');
  });

  it('matches tracks on artist too and highlights only the title', () => {
    const g = buildPaletteGroups('tale', sources());
    const item = g.find((x) => x.key === 'tracks')!.items[0];
    expect(item.id).toBe('track:2');
    expect(item.ranges ?? []).toEqual([]);
  });

  it('runs the matching entry', () => {
    const run = vi.fn();
    buildPaletteGroups('new', sources(run))[0].items[0].run();
    expect(run).toHaveBeenCalledTimes(1);
  });
});
