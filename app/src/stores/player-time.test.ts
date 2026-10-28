import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('../services/playback.svelte.ts', () => ({
  playbackPause: vi.fn(),
  playbackPlay: vi.fn(),
  playbackSeek: vi.fn(),
  playbackSetPosition: vi.fn(),
  playbackStop: vi.fn(),
  subscribePlaybackError: vi.fn(async () => () => {}),
  subscribePlaybackTick: vi.fn(async () => () => {}),
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import { currentTime, currentTimeMs, player } from './player.svelte.ts';

function loadAt(totalSeconds: number, seconds: number) {
  player.track = { totalSeconds, durationMs: totalSeconds * 1000 } as typeof player.track;
  player.progress = seconds / totalSeconds;
}

afterEach(() => {
  player.track = null;
  player.progress = 0;
});

describe('time readout tenths', () => {
  it('shows tenths of the second, not of the track progress', () => {
    loadAt(300, 83.7);
    expect(currentTime()).toBe('1:23');
    expect(currentTimeMs()).toBe('.7');
  });

  it('does not depend on how far into the track it is', () => {
    loadAt(300, 5.2);
    expect(currentTimeMs()).toBe('.2');
    loadAt(60, 5.2);
    expect(currentTimeMs()).toBe('.2');
  });

  it('rolls to .0 on the whole second', () => {
    loadAt(240, 10);
    expect(currentTimeMs()).toBe('.0');
  });

  it('is .0 without a track', () => {
    expect(currentTimeMs()).toBe('.0');
  });
});
