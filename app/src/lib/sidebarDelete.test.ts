import { describe, expect, it } from 'vitest';
import { canDeleteSidebarSelection } from './sidebarDelete.ts';
import { buildAppActions } from '../shortcuts/app-actions.ts';
import { createShortcutRegistry } from '../shortcuts/registry.svelte.ts';

describe('canDeleteSidebarSelection', () => {
  it('needs a selection and the sidebar to have focus', () => {
    expect(canDeleteSidebarSelection([3], true)).toBe(true);
    expect(canDeleteSidebarSelection([], true)).toBe(false);
    expect(canDeleteSidebarSelection([3, 4], false)).toBe(false);
  });
});

describe('playlist delete shortcut', () => {
  const actions = buildAppActions();

  it('binds Backspace and Delete in the registry', () => {
    expect(actions.find((a) => a.id === 'library.playlists.delete')?.defaultBinding).toBe('Backspace');
    expect(actions.find((a) => a.id === 'library.playlists.deleteForward')?.defaultBinding).toBe('Delete');
  });

  it('does not fire while typing in an input', () => {
    const registry = createShortcutRegistry(actions, null);
    const event = { key: 'Backspace', code: 'Backspace', preventDefault: () => {} };
    expect(registry.dispatch(event, { typing: true })).toBe(false);
  });
});
