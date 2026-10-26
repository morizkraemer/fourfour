import { beforeEach, describe, expect, it, vi } from 'vitest';

const hooks = vi.hoisted(() => ({
  errorHandler: null as null | ((e: { message: string; path?: string | null }) => void),
  tickHandler: null as null | ((t: Record<string, unknown>) => void),
}));

vi.mock('../services/playback.svelte.ts', () => ({
  playbackPause: vi.fn(),
  playbackPlay: vi.fn(),
  playbackSeek: vi.fn(),
  playbackSetPosition: vi.fn(),
  playbackStop: vi.fn(),
  subscribePlaybackError: vi.fn(async (handler) => {
    hooks.errorHandler = handler;
    return () => {};
  }),
  subscribePlaybackTick: vi.fn(async (handler) => {
    hooks.tickHandler = handler;
    return () => {};
  }),
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import { initPlayerTransport, player } from './player.svelte.ts';

beforeEach(async () => {
  await initPlayerTransport();
  player.error = null;
  player.playing = false;
});

describe('playback-error handling', () => {
  it('subscribes to backend playback errors on transport init', () => {
    expect(hooks.errorHandler).toBeTypeOf('function');
    expect(hooks.tickHandler).toBeTypeOf('function');
  });
  it('stores the message and stops the playing flag', () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    player.playing = true;
    hooks.errorHandler!({ message: 'device lost', path: '/a.mp3' });
    expect(player.error).toBe('device lost');
    expect(player.playing).toBe(false);
  });
  it('keeps the latest error when several arrive', () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    hooks.errorHandler!({ message: 'first' });
    hooks.errorHandler!({ message: 'second' });
    expect(player.error).toBe('second');
  });
  it('logs the failure to the console', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    hooks.errorHandler!({ message: 'decode failed' });
    expect(spy).toHaveBeenCalledWith('playback error:', 'decode failed');
  });
  it('tick updates levels even without a loaded track', () => {
    hooks.tickHandler!({ position_ms: 0, duration_ms: 0, playing: true, level_left: 0.4, level_right: 0.6 });
    expect(player.levelLeft).toBe(0.4);
    expect(player.levelRight).toBe(0.6);
    expect(player.playing).toBe(false);
  });
});
