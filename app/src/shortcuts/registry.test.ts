import { describe, expect, it, vi } from 'vitest';
import { createShortcutRegistry, SHORTCUT_STORAGE_KEY, type ShortcutAction, type StorageLike } from './registry.svelte.ts';

function memoryStorage(initial: Record<string, string> = {}): StorageLike & { data: Record<string, string> } {
  const data = { ...initial };
  return {
    data,
    getItem: (k) => data[k] ?? null,
    setItem: (k, v) => {
      data[k] = v;
    },
  };
}

function fixture() {
  const calls: string[] = [];
  const act = (id: string, defaultBinding: string | null, extra: Partial<ShortcutAction> = {}): ShortcutAction => ({
    id,
    label: id,
    group: 'G',
    defaultBinding,
    run: () => calls.push(id),
    ...extra,
  });
  const actions = [
    act('player', 'Tab'),
    act('drawer', 'Shift+Tab'),
    act('play', 'Space'),
    act('palette', 'Mod+K'),
    act('unbound', null),
  ];
  return { calls, actions };
}

function key(init: { key: string; code?: string; ctrlKey?: boolean; shiftKey?: boolean; metaKey?: boolean }) {
  return { ...init, preventDefault: vi.fn() };
}

describe('defaults and lookup', () => {
  it('exposes default bindings', () => {
    const { actions } = fixture();
    const reg = createShortcutRegistry(actions, null);
    expect(reg.bindingOf('player')).toBe('Tab');
    expect(reg.bindingOf('unbound')).toBeNull();
    expect(reg.isCustom('player')).toBe(false);
  });

  it('groups actions in registration order', () => {
    const reg = createShortcutRegistry(
      [
        { id: 'a', label: 'a', group: 'One', defaultBinding: null, run: () => {} },
        { id: 'b', label: 'b', group: 'Two', defaultBinding: null, run: () => {} },
        { id: 'c', label: 'c', group: 'One', defaultBinding: null, run: () => {} },
      ],
      null,
    );
    expect(reg.groups().map((g) => [g.group, g.actions.map((a) => a.id)])).toEqual([
      ['One', ['a', 'c']],
      ['Two', ['b']],
    ]);
  });
});

describe('rebinding', () => {
  it('rebinds and dispatches the new key, not the old one', () => {
    const { actions, calls } = fixture();
    const reg = createShortcutRegistry(actions, null);
    expect(reg.rebind('play', 'P')).toEqual({ ok: true });
    expect(reg.dispatch(key({ key: 'p', code: 'KeyP' }), { typing: false })).toBe(true);
    expect(reg.dispatch(key({ key: ' ', code: 'Space' }), { typing: false })).toBe(false);
    expect(calls).toEqual(['play']);
  });

  it('rejects a conflicting binding with a message naming the other action', () => {
    const { actions } = fixture();
    const reg = createShortcutRegistry(actions, null);
    const result = reg.rebind('play', 'Tab');
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.conflict.id).toBe('player');
      expect(result.message).toContain('already used by “player”');
    }
    expect(reg.bindingOf('play')).toBe('Space');
  });

  it('replace unbinds the conflicting action', () => {
    const { actions } = fixture();
    const reg = createShortcutRegistry(actions, null);
    expect(reg.rebind('play', 'Tab', { replace: true })).toEqual({ ok: true });
    expect(reg.bindingOf('play')).toBe('Tab');
    expect(reg.bindingOf('player')).toBeNull();
  });

  it('allows rebinding an action to its own current binding', () => {
    const reg = createShortcutRegistry(fixture().actions, null);
    expect(reg.rebind('play', 'Space')).toEqual({ ok: true });
  });

  it('reset restores one default, resetAll restores everything', () => {
    const reg = createShortcutRegistry(fixture().actions, null);
    reg.rebind('play', 'P');
    reg.rebind('drawer', 'D');
    reg.reset('play');
    expect(reg.bindingOf('play')).toBe('Space');
    expect(reg.bindingOf('drawer')).toBe('D');
    reg.resetAll();
    expect(reg.bindingOf('drawer')).toBe('Shift+Tab');
    expect(reg.hasCustom()).toBe(false);
  });

  it('unbind clears the binding', () => {
    const reg = createShortcutRegistry(fixture().actions, null);
    reg.unbind('play');
    expect(reg.bindingOf('play')).toBeNull();
    expect(reg.isCustom('play')).toBe(true);
  });
});

describe('persistence', () => {
  it('writes only overrides and restores them in a new registry', () => {
    const storage = memoryStorage();
    const first = createShortcutRegistry(fixture().actions, storage);
    first.rebind('play', 'P');
    expect(JSON.parse(storage.data[SHORTCUT_STORAGE_KEY])).toEqual({ play: 'P' });
    const second = createShortcutRegistry(fixture().actions, storage);
    expect(second.bindingOf('play')).toBe('P');
    expect(second.bindingOf('player')).toBe('Tab');
  });

  it('drops stored entries for unknown actions and survives corrupt JSON', () => {
    const stale = memoryStorage({ [SHORTCUT_STORAGE_KEY]: JSON.stringify({ gone: 'X', play: 'P' }) });
    expect(createShortcutRegistry(fixture().actions, stale).bindingOf('play')).toBe('P');
    const corrupt = memoryStorage({ [SHORTCUT_STORAGE_KEY]: '{nope' });
    expect(createShortcutRegistry(fixture().actions, corrupt).bindingOf('play')).toBe('Space');
  });

  it('setting a binding back to its default removes the override', () => {
    const storage = memoryStorage();
    const reg = createShortcutRegistry(fixture().actions, storage);
    reg.rebind('play', 'P');
    reg.rebind('play', 'Space');
    expect(JSON.parse(storage.data[SHORTCUT_STORAGE_KEY])).toEqual({});
  });
});

describe('dispatch', () => {
  it('runs a Mod binding from either Ctrl or Meta', () => {
    const { actions, calls } = fixture();
    const reg = createShortcutRegistry(actions, null);
    reg.dispatch(key({ key: 'k', code: 'KeyK', ctrlKey: true }), { typing: false });
    reg.dispatch(key({ key: 'k', code: 'KeyK', metaKey: true }), { typing: false });
    expect(calls).toEqual(['palette', 'palette']);
  });

  it('Tab toggles the player and Shift+Tab the drawer, preventing default', () => {
    const { actions, calls } = fixture();
    const reg = createShortcutRegistry(actions, null);
    const tab = key({ key: 'Tab', code: 'Tab' });
    const shiftTab = key({ key: 'Tab', code: 'Tab', shiftKey: true });
    expect(reg.dispatch(tab, { typing: false })).toBe(true);
    expect(reg.dispatch(shiftTab, { typing: false })).toBe(true);
    expect(calls).toEqual(['player', 'drawer']);
    expect(tab.preventDefault).toHaveBeenCalled();
    expect(shiftTab.preventDefault).toHaveBeenCalled();
  });

  it('leaves Tab and Shift+Tab alone while typing in a text field', () => {
    const { actions, calls } = fixture();
    const reg = createShortcutRegistry(actions, null);
    const tab = key({ key: 'Tab', code: 'Tab' });
    const shiftTab = key({ key: 'Tab', code: 'Tab', shiftKey: true });
    expect(reg.dispatch(tab, { typing: true })).toBe(false);
    expect(reg.dispatch(shiftTab, { typing: true })).toBe(false);
    expect(reg.dispatch(key({ key: ' ', code: 'Space' }), { typing: true })).toBe(false);
    expect(calls).toEqual([]);
    expect(tab.preventDefault).not.toHaveBeenCalled();
  });

  it('still fires Mod and Escape bindings while typing, unless the action opts out', () => {
    const calls: string[] = [];
    const reg = createShortcutRegistry(
      [
        { id: 'palette', label: 'p', group: 'G', defaultBinding: 'Mod+K', run: () => calls.push('palette') },
        { id: 'esc', label: 'e', group: 'G', defaultBinding: 'Escape', run: () => calls.push('esc') },
        { id: 'open', label: 'o', group: 'G', defaultBinding: 'Mod+O', inInput: false, run: () => calls.push('open') },
      ],
      null,
    );
    reg.dispatch(key({ key: 'k', code: 'KeyK', ctrlKey: true }), { typing: true });
    reg.dispatch(key({ key: 'Escape', code: 'Escape' }), { typing: true });
    reg.dispatch(key({ key: 'o', code: 'KeyO', ctrlKey: true }), { typing: true });
    expect(calls).toEqual(['palette', 'esc']);
  });

  it('skips actions whose when() is false and falls through to the next match', () => {
    const calls: string[] = [];
    const reg = createShortcutRegistry(
      [
        { id: 'first', label: 'f', group: 'G', defaultBinding: 'Tab', when: () => false, run: () => calls.push('first') },
        { id: 'second', label: 's', group: 'G', defaultBinding: null, run: () => calls.push('second') },
      ],
      null,
    );
    const tab = key({ key: 'Tab', code: 'Tab' });
    expect(reg.dispatch(tab, { typing: false })).toBe(false);
    expect(tab.preventDefault).not.toHaveBeenCalled();
    expect(calls).toEqual([]);
  });

  it('ignores bare modifier presses', () => {
    const reg = createShortcutRegistry(fixture().actions, null);
    expect(reg.dispatch(key({ key: 'Shift', shiftKey: true }), { typing: false })).toBe(false);
  });
});
