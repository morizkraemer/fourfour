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

  it('adds a hot cue in the next free pad on Shift+C', () => {
    const action = actions.find((a) => a.id === 'cues.hot.add');
    expect(action?.defaultBinding).toBe('Shift+C');
    expect(bindingFromEvent({ key: 'C', code: 'KeyC', shiftKey: true })).toBe('Shift+C');
  });

  it('offers set, jump and delete for every pad in the palette', () => {
    for (let slot = 1; slot <= 8; slot++) {
      const letter = String.fromCharCode(64 + slot);
      for (const [suffix, label] of [
        ['.set', `Set hot cue ${letter} at playhead`],
        ['.jump', `Jump to hot cue ${letter}`],
        ['.delete', `Delete hot cue ${letter}`],
      ]) {
        const action = actions.find((a) => a.id === `cues.hot${slot}${suffix}`);
        expect(action?.label).toBe(label);
        expect(action?.palette).not.toBe(false);
      }
    }
    for (const id of ['cues.hot.add', 'cues.memory.add', 'cues.clearAll']) {
      expect(actions.find((a) => a.id === id)?.palette).not.toBe(false);
    }
  });

  it('offers the player and grid actions in the palette', () => {
    const ids = ['wave.zoomIn', 'wave.zoomOut', 'wave.zoomFit', 'grid.setDownbeat', 'grid.nudgeLeft', 'grid.nudgeRight', 'tempo.masterTempo', 'tempo.keyUp', 'tempo.keyDown', 'library.loadSelected', 'library.revealSelected'];
    for (const id of ids) {
      const action = actions.find((a) => a.id === id);
      expect(action, id).toBeDefined();
      expect(action?.palette).not.toBe(false);
    }
  });

  it('gives no two actions the same binding', () => {
    const bindings = actions
      .filter((a) => !a.id.endsWith('.digit'))
      .map((a) => a.defaultBinding)
      .filter((b): b is string => b !== null);
    expect(new Set(bindings).size).toBe(bindings.length);
  });

  it('shares the plain 1 to 8 keys between hot cues and favourites, 9 stays a favourite', () => {
    for (let slot = 1; slot <= 9; slot++) {
      expect(actions.find((a) => a.id === `favorites.toggle${slot}`)?.defaultBinding).toBe(String(slot));
    }
    const digitCues = cueActions.filter((a) => a.defaultBinding?.match(/^\d$/));
    expect(digitCues.map((a) => a.id)).toEqual(Array.from({ length: 8 }, (_, i) => `cues.hot${i + 1}.digit`));
    digitCues.forEach((a, i) => expect(a.defaultBinding).toBe(String(i + 1)));
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
