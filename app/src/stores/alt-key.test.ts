import { afterEach, describe, expect, it, vi } from 'vitest';

type Listener = (e: { altKey?: boolean }) => void;

async function loadWithWindow() {
  vi.resetModules();
  const listeners = new Map<string, Listener>();
  vi.stubGlobal('window', { addEventListener: (type: string, fn: Listener) => listeners.set(type, fn) });
  const { altKey } = await import('./alt-key.svelte.ts');
  return { altKey, fire: (type: string, altHeld = false) => listeners.get(type)!({ altKey: altHeld }) };
}

afterEach(() => vi.unstubAllGlobals());

describe('altKey', () => {
  it('follows the key on keydown and keyup', async () => {
    const { altKey, fire } = await loadWithWindow();
    expect(altKey.held).toBe(false);
    fire('keydown', true);
    expect(altKey.held).toBe(true);
    fire('keyup', false);
    expect(altKey.held).toBe(false);
  });

  it('lets go when the window loses focus', async () => {
    const { altKey, fire } = await loadWithWindow();
    fire('keydown', true);
    fire('blur');
    expect(altKey.held).toBe(false);
  });
});
