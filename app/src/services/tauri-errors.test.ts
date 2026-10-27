import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import { changeLibraryPath, getLibraryPath, readUsbState } from './tauri.svelte.ts';

describe('inside the real app', () => {
  beforeEach(() => {
    vi.stubGlobal('window', { __TAURI_INTERNALS__: {} });
    invokeMock.mockRejectedValue(new Error('backend down'));
  });
  afterEach(() => {
    vi.unstubAllGlobals();
    invokeMock.mockReset();
  });

  it('getLibraryPath rethrows instead of returning a mock path', async () => {
    await expect(getLibraryPath()).rejects.toThrow('backend down');
  });

  it('changeLibraryPath rethrows instead of reporting success', async () => {
    await expect(changeLibraryPath('/music')).rejects.toThrow('backend down');
  });

  it('readUsbState rethrows instead of returning a fake track', async () => {
    await expect(readUsbState('/media/usb')).rejects.toThrow('backend down');
  });
});

describe('in a browser preview', () => {
  beforeEach(() => {
    invokeMock.mockRejectedValue(new Error('no tauri'));
  });
  afterEach(() => invokeMock.mockReset());

  it('still serves mock data', async () => {
    expect(await getLibraryPath()).toBe('/mock/user/library.db');
    expect(await changeLibraryPath('/music')).toBe('/music/library.db');
    expect((await readUsbState('/media/usb'))?.tracks[0].title).toBe('USB Track 1');
  });
});
