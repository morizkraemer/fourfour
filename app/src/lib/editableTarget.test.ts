import { describe, expect, it } from 'vitest';
import { isEditableTarget, isKeyboardFocusedControl } from './editableTarget.ts';

function element(tagName: string, matchingSelectorPart: string | null = null) {
  return {
    tagName,
    closest: (selector: string) =>
      matchingSelectorPart && selector.includes(matchingSelectorPart) ? {} : null,
  };
}

describe('isEditableTarget', () => {
  it('treats input, textarea and select as typing', () => {
    expect(isEditableTarget(element('INPUT'))).toBe(true);
    expect(isEditableTarget(element('TEXTAREA'))).toBe(true);
    expect(isEditableTarget(element('SELECT'))).toBe(true);
  });

  it('treats empty and true contenteditable as typing', () => {
    expect(isEditableTarget(element('DIV', 'contenteditable=""'))).toBe(true);
    expect(isEditableTarget(element('DIV', 'contenteditable="true"'))).toBe(true);
  });

  it('treats dialogs and settings as typing', () => {
    expect(isEditableTarget(element('DIV', '.ff-dialog-wrapper'))).toBe(true);
    expect(isEditableTarget(element('DIV', '.ff-settings-screen'))).toBe(true);
  });

  it('ignores plain elements and non-elements', () => {
    expect(isEditableTarget(element('DIV'))).toBe(false);
    expect(isEditableTarget(null)).toBe(false);
    expect(isEditableTarget({})).toBe(false);
  });
});

describe('isKeyboardFocusedControl', () => {
  const focusVisible = (visible: boolean) => ({ matches: (selector: string) => visible && selector === ':focus-visible' });

  it('is true only for keyboard-focused elements', () => {
    expect(isKeyboardFocusedControl(focusVisible(true))).toBe(true);
    expect(isKeyboardFocusedControl(focusVisible(false))).toBe(false);
  });

  it('is false for non-elements', () => {
    expect(isKeyboardFocusedControl(null)).toBe(false);
    expect(isKeyboardFocusedControl({})).toBe(false);
  });
});
