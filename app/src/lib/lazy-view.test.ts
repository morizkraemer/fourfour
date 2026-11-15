import { afterEach, describe, expect, it, vi } from 'vitest';
import { lazyView } from './lazy.svelte.ts';
import { firstScreenShown } from './afterFirstScreen.ts';

const Stub = (() => {}) as never;

describe('lazyView', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('has no component before it is loaded and imports once however often it is asked', async () => {
    const importView = vi.fn(async () => ({ default: Stub }));
    const view = lazyView(importView);
    expect(view.component).toBeNull();

    await Promise.all([view.load(), view.load()]);
    await view.load();

    expect(importView).toHaveBeenCalledOnce();
    expect(view.component).toBe(Stub);
  });

  it('tries again on the next request after a failed import', async () => {
    const importView = vi.fn().mockRejectedValueOnce(new Error('offline')).mockResolvedValue({ default: Stub });
    const view = lazyView(importView);

    await expect(view.load()).rejects.toThrow('offline');
    expect(view.component).toBeNull();
    await view.load();

    expect(importView).toHaveBeenCalledTimes(2);
    expect(view.component).toBe(Stub);
  });

  it('loads on its own once the first screen has been shown', async () => {
    const importView = vi.fn(async () => ({ default: Stub }));
    const view = lazyView(importView);
    expect(importView).not.toHaveBeenCalled();

    vi.stubGlobal('requestAnimationFrame', (run: () => void) => setTimeout(run, 0));
    firstScreenShown();
    await vi.waitFor(() => expect(view.component).toBe(Stub), { timeout: 5000 });

    expect(importView).toHaveBeenCalledOnce();
  });
});
