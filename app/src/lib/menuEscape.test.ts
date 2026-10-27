import { describe, expect, it, vi } from 'vitest';
import { closeOnEscape } from '../../../prototyping/viewport/design-system/components/menuEscape.js';

function keyEvent(key: string) {
  return { key, preventDefault: vi.fn(), stopPropagation: vi.fn() };
}

describe('closeOnEscape', () => {
  it('closes and keeps Escape from reaching global shortcuts', () => {
    const event = keyEvent('Escape');
    const onClose = vi.fn();
    expect(closeOnEscape(event, onClose)).toBe(true);
    expect(onClose).toHaveBeenCalledOnce();
    expect(event.preventDefault).toHaveBeenCalledOnce();
    expect(event.stopPropagation).toHaveBeenCalledOnce();
  });

  it('ignores other keys', () => {
    const event = keyEvent('a');
    const onClose = vi.fn();
    expect(closeOnEscape(event, onClose)).toBe(false);
    expect(onClose).not.toHaveBeenCalled();
    expect(event.preventDefault).not.toHaveBeenCalled();
  });
});
