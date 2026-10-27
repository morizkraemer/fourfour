import { describe, expect, it, vi } from 'vitest';
import { unlistenWhenReady } from './lateUnlisten.ts';

describe('unlistenWhenReady', () => {
  it('unlistens once the subscription resolves if cleanup ran first', async () => {
    const unlisten = vi.fn();
    let resolve!: (fn: () => void) => void;
    const cleanup = unlistenWhenReady(new Promise((r) => (resolve = r)), vi.fn());
    cleanup();
    resolve(unlisten);
    await Promise.resolve();
    await Promise.resolve();
    expect(unlisten).toHaveBeenCalledOnce();
  });

  it('unlistens immediately when already subscribed, and only once', async () => {
    const unlisten = vi.fn();
    const cleanup = unlistenWhenReady(Promise.resolve(unlisten), vi.fn());
    await Promise.resolve();
    cleanup();
    cleanup();
    expect(unlisten).toHaveBeenCalledOnce();
  });

  it('reports a failed subscription', async () => {
    const onError = vi.fn();
    unlistenWhenReady(Promise.reject(new Error('nope')), onError);
    await Promise.resolve();
    await Promise.resolve();
    expect(onError).toHaveBeenCalledOnce();
  });
});
