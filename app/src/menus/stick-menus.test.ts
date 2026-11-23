import { describe, expect, it } from 'vitest';
import { buildStickPlaylistMenuItems, buildStickTrackMenuItems, stickTrackRemoveLabel } from './context-menus.ts';

function trackMenu(overrides: Partial<Parameters<typeof buildStickTrackMenuItems>[0]> = {}) {
  const calls: string[] = [];
  const items = buildStickTrackMenuItems({
    scope: 'playlist',
    count: 1,
    canEditPlaylist: true,
    onRemove: () => calls.push('remove'),
    onShowInLibrary: () => calls.push('show'),
    ...overrides,
  });
  return { items, calls, labels: items.filter((i) => !i.isSeparator).map((i) => i.label) };
}

describe('stick track menu', () => {
  it('names where the track is removed from, never the library', () => {
    expect(stickTrackRemoveLabel('all', 1)).toBe('Remove from stick');
    expect(stickTrackRemoveLabel('all', 3)).toBe('Remove 3 from stick');
    expect(stickTrackRemoveLabel('playlist', 1)).toBe('Remove from this playlist on the stick');
    expect(stickTrackRemoveLabel('playlist', 2)).toBe('Remove 2 from this playlist on the stick');
  });

  it('offers Show in library and the removal for one track', () => {
    const { items, calls, labels } = trackMenu();
    expect(labels).toEqual(['Show in library', 'Remove from this playlist on the stick']);
    items.forEach((i) => i.onClick?.());
    expect(calls).toEqual(['show', 'remove']);
  });

  it('shows one track at a time in the library', () => {
    const { items } = trackMenu({ count: 2 });
    expect(items[0].disabled).toBe(true);
    expect(items.at(-1)!.disabled).toBe(false);
  });

  it('cannot edit a playlist when the engine keeps no stick list, but All tracks still can', () => {
    expect(trackMenu({ canEditPlaylist: false }).items.at(-1)!.disabled).toBe(true);
    expect(trackMenu({ canEditPlaylist: false, scope: 'all' }).items.at(-1)!.disabled).toBe(false);
  });

  it('is empty with nothing selected', () => {
    expect(trackMenu({ count: 0 }).items).toEqual([]);
  });
});

describe('stick playlist menu', () => {
  it('opens, shows in library, or takes the playlist off the stick', () => {
    const calls: string[] = [];
    const items = buildStickPlaylistMenuItems({
      onOpen: () => calls.push('open'),
      onShowInLibrary: () => calls.push('show'),
      onTakeOff: () => calls.push('off'),
    });
    expect(items.filter((i) => !i.isSeparator).map((i) => i.label)).toEqual(['Open', 'Show in library', 'Take off this stick']);
    items.forEach((i) => i.onClick?.());
    expect(calls).toEqual(['open', 'show', 'off']);
  });
});
