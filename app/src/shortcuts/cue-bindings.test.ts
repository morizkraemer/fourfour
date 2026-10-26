import { describe, expect, it } from 'vitest';
import { buildAppActions } from './app-actions.ts';
import { bindingFromEvent } from './keybinding.ts';

const actions = buildAppActions();
const cueActions = actions.filter((a) => a.group === 'Cues');

describe('cue shortcuts', () => {
  it('binds Shift+1 to Shift+8 to hot cues A to H', () => {
    for (let slot = 1; slot <= 8; slot++) {
      const action = actions.find((a) => a.id === `cues.hot${slot}`);
      expect(action?.defaultBinding).toBe(`Shift+${slot}`);
      expect(action?.label).toContain(`Hot cue ${String.fromCharCode(64 + slot)}`);
    }
  });

  it('binds Alt+Shift+1 to Alt+Shift+8 to deleting them', () => {
    for (let slot = 1; slot <= 8; slot++) {
      expect(actions.find((a) => a.id === `cues.hot${slot}.delete`)?.defaultBinding).toBe(`Alt+Shift+${slot}`);
    }
  });

  it('has CUE on C and a memory cue on M', () => {
    expect(actions.find((a) => a.id === 'cues.cue')?.defaultBinding).toBe('C');
    expect(actions.find((a) => a.id === 'cues.memory.add')?.defaultBinding).toBe('M');
  });

  it('gives no two actions the same binding', () => {
    const bindings = actions.map((a) => a.defaultBinding).filter((b): b is string => b !== null);
    expect(new Set(bindings).size).toBe(bindings.length);
  });

  it('keeps the plain 1 to 9 keys for favourites', () => {
    for (const action of cueActions) expect(action.defaultBinding).not.toMatch(/^\d$/);
    for (let slot = 1; slot <= 9; slot++) {
      expect(actions.find((a) => a.id === `favorites.toggle${slot}`)?.defaultBinding).toBe(String(slot));
    }
  });

  it('matches a Shift+digit key press whatever character the layout types', () => {
    expect(bindingFromEvent({ key: '!', code: 'Digit1', shiftKey: true })).toBe('Shift+1');
    expect(bindingFromEvent({ key: '"', code: 'Digit2', shiftKey: true })).toBe('Shift+2');
    expect(bindingFromEvent({ key: '¡', code: 'Digit1', shiftKey: true, altKey: true })).toBe('Alt+Shift+1');
  });

  it('is available only with a loaded track', () => {
    for (const action of cueActions) expect(action.when).toBeTypeOf('function');
    for (const action of cueActions) expect(action.when!()).toBe(false);
  });
});
