import { describe, expect, it, vi } from 'vitest';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import { library, refreshUsbState } from './library.svelte.ts';

function stickState(title: string) {
  return {
    tracks: [{ id: 1, title, artist: 'A', album: 'B', genre: '', key: '8A', bpm: 128, duration: 300, usb_path: '/x.mp3' }],
    playlists: [],
  };
}

describe('refreshUsbState', () => {
  it('keeps the newer stick when an older read resolves last', async () => {
    const pending = new Map<string, (value: unknown) => void>();
    invokeMock.mockImplementation((cmd: string, args: { path: string }) => {
      if (cmd !== 'read_usb_state') return Promise.resolve(null);
      return new Promise((resolve) => pending.set(args.path, resolve));
    });

    library.activeVolume = '/Volumes/A';
    const first = refreshUsbState();
    library.activeVolume = '/Volumes/B';
    const second = refreshUsbState();

    pending.get('/Volumes/B')!(stickState('On B'));
    await second;
    pending.get('/Volumes/A')!(stickState('On A'));
    await first;

    expect(library.usbTracks.map((t) => t.title)).toEqual(['On B']);
  });
});
