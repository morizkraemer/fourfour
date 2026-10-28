import { describe, expect, it } from 'vitest';
import { buildAppActions } from './app-actions.ts';

const actions = buildAppActions();
const bindingOf = (id: string) => actions.find((a) => a.id === id)?.defaultBinding;

describe('search and command shortcuts', () => {
  it('opens the one dropdown from Mod+F, Mod+K and Alt+Enter', () => {
    expect(bindingOf('library.search')).toBe('Mod+F');
    expect(bindingOf('palette.toggle')).toBe('Mod+K');
    expect(bindingOf('palette.open')).toBe('Alt+Enter');
  });

  it('lets Alt+Enter fire while typing in the search field', () => {
    expect(actions.find((a) => a.id === 'palette.open')?.inInput).toBe(true);
  });
});
