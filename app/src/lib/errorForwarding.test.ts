import { describe, expect, it, vi } from 'vitest';
import { describeValue, installErrorForwarding } from './errorForwarding.ts';

function fakeWindow() {
  const listeners: Record<string, (event: any) => void> = {};
  const originalError = vi.fn();
  return {
    console: { error: originalError },
    originalError,
    listeners,
    addEventListener: (type: string, fn: (event: any) => void) => {
      listeners[type] = fn;
    },
  };
}

describe('installErrorForwarding', () => {
  it('forwards console.error and still calls the original', () => {
    const win = fakeWindow();
    const original = win.originalError;
    const send = vi.fn().mockResolvedValue(undefined);
    installErrorForwarding(win, send);
    win.console.error('load failed', { id: 3 });
    expect(original).toHaveBeenCalledWith('load failed', { id: 3 });
    expect(send).toHaveBeenCalledWith('error', 'load failed {"id":3}');
  });

  it('forwards uncaught errors and unhandled rejections', () => {
    const win = fakeWindow();
    const send = vi.fn().mockResolvedValue(undefined);
    installErrorForwarding(win, send);
    win.listeners.error({ message: 'boom', filename: 'app.js', lineno: 4, colno: 9 });
    win.listeners.unhandledrejection({ reason: 'no luck' });
    expect(send).toHaveBeenNthCalledWith(1, 'error', 'uncaught boom at app.js:4:9');
    expect(send).toHaveBeenNthCalledWith(2, 'error', 'unhandled rejection: no luck');
  });

  it('rate limits a burst and resumes after the window', () => {
    const win = fakeWindow();
    const send = vi.fn().mockResolvedValue(undefined);
    let clock = 1000;
    installErrorForwarding(win, send, () => clock);
    for (let n = 0; n < 50; n++) win.console.error(`line ${n}`);
    expect(send).toHaveBeenCalledTimes(20);
    clock += 10_000;
    win.console.error('later');
    expect(send).toHaveBeenCalledTimes(21);
  });

  it('survives a failing or throwing backend call without raising', async () => {
    const win = fakeWindow();
    const original = win.originalError;
    const send = vi
      .fn()
      .mockRejectedValueOnce(new Error('ipc down'))
      .mockImplementationOnce(() => {
        throw new Error('sync');
      });
    installErrorForwarding(win, send);
    expect(() => win.console.error('a')).not.toThrow();
    expect(() => win.console.error('b')).not.toThrow();
    await Promise.resolve();
    expect(original).toHaveBeenCalledTimes(2);
  });
});

describe('describeValue', () => {
  it('uses the stack of an Error and survives circular objects', () => {
    expect(describeValue(new Error('x'))).toContain('Error: x');
    const loop: Record<string, unknown> = {};
    loop.self = loop;
    expect(describeValue(loop)).toBe('[object Object]');
  });
});
