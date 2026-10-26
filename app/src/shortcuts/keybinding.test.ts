import { describe, expect, it } from 'vitest';
import { bindingFromEvent, bindingParts, firesWhileTyping, formatBinding } from './keybinding.ts';

describe('bindingFromEvent', () => {
  it('builds canonical bindings with ordered modifiers', () => {
    expect(bindingFromEvent({ key: 'k', code: 'KeyK', metaKey: true })).toBe('Mod+K');
    expect(bindingFromEvent({ key: 'N', code: 'KeyN', ctrlKey: true, shiftKey: true })).toBe('Mod+Shift+N');
    expect(bindingFromEvent({ key: 'x', code: 'KeyX', altKey: true, shiftKey: true, ctrlKey: true })).toBe('Mod+Alt+Shift+X');
  });
  it('names Tab, Space, Escape, digits and punctuation', () => {
    expect(bindingFromEvent({ key: 'Tab', code: 'Tab', shiftKey: true })).toBe('Shift+Tab');
    expect(bindingFromEvent({ key: ' ', code: 'Space' })).toBe('Space');
    expect(bindingFromEvent({ key: 'Escape', code: 'Escape' })).toBe('Escape');
    expect(bindingFromEvent({ key: '!', code: 'Digit1', shiftKey: true })).toBe('Shift+1');
    expect(bindingFromEvent({ key: ',', code: 'Comma', metaKey: true })).toBe('Mod+,');
  });
  it('returns null for a bare modifier', () => {
    expect(bindingFromEvent({ key: 'Control', ctrlKey: true })).toBeNull();
    expect(bindingFromEvent({ key: 'Shift', shiftKey: true })).toBeNull();
  });
});

describe('display', () => {
  it('formats for mac and other platforms', () => {
    expect(formatBinding('Mod+Shift+N', true)).toBe('⌘⇧N');
    expect(formatBinding('Mod+Shift+N', false)).toBe('Ctrl+Shift+N');
    expect(formatBinding('Escape', false)).toBe('Esc');
    expect(formatBinding(null, true)).toBe('');
    expect(bindingParts('Mod+K', false)).toEqual(['Ctrl', 'K']);
  });
});

describe('firesWhileTyping', () => {
  it('allows Mod, Escape and function keys only', () => {
    expect(firesWhileTyping('Mod+K')).toBe(true);
    expect(firesWhileTyping('Escape')).toBe(true);
    expect(firesWhileTyping('F6')).toBe(true);
    expect(firesWhileTyping('Shift+F6')).toBe(true);
    expect(firesWhileTyping('Tab')).toBe(false);
    expect(firesWhileTyping('Shift+Tab')).toBe(false);
    expect(firesWhileTyping('Space')).toBe(false);
    expect(firesWhileTyping('1')).toBe(false);
  });
});
