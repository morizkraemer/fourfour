import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

const tracks = { tracks: [], playlists: [] };

async function freshTauriService() {
  vi.resetModules();
  return import('./tauri.svelte.ts');
}

describe('load_state sent by index.html', () => {
  let page: { __TAURI_INTERNALS__: object; __ffLoadStateStarted?: Promise<unknown> };

  beforeEach(() => {
    page = { __TAURI_INTERNALS__: {} };
    vi.stubGlobal('window', page);
    invokeMock.mockReset();
  });
  afterEach(() => vi.unstubAllGlobals());

  it('is the call loadState waits for, so the backend gets one request', async () => {
    page.__ffLoadStateStarted = Promise.resolve(tracks);
    const { startLoadStateEarly, loadState } = await freshTauriService();

    startLoadStateEarly();
    const state = await loadState();

    expect(invokeMock).not.toHaveBeenCalled();
    expect(state).toEqual(tracks);
    expect(page.__ffLoadStateStarted).toBeUndefined();
  });

  it('fails with the typed error the bundle sends itself', async () => {
    page.__ffLoadStateStarted = Promise.reject({ kind: 'library_locked', message: 'in use', path: '/lib' });
    const { startLoadStateEarly, loadState } = await freshTauriService();

    startLoadStateEarly();
    const failure = await loadState().catch((err) => err);

    expect(failure).toMatchObject({ name: 'CommandError', kind: 'library_locked', message: 'in use', path: '/lib' });
  });

  it('is sent by the bundle when the page did not send it', async () => {
    invokeMock.mockResolvedValue(tracks);
    const { startLoadStateEarly, loadState } = await freshTauriService();

    startLoadStateEarly();
    await loadState();

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith('load_state');
  });

  it('decodes a binary answer into the tracks and playlists of the JSON answer', async () => {
    const bytes = readFileSync(resolve(process.cwd(), 'src/services/fixtures/state-edge.bin'));
    const json = JSON.parse(readFileSync(resolve(process.cwd(), 'src/services/fixtures/state-edge.json'), 'utf8'));
    invokeMock.mockResolvedValue(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength));
    const { loadState } = await freshTauriService();
    const { decodeTrackTable } = await import('./track-table.ts');

    const state = await loadState();

    expect(state.tracks).toStrictEqual(decodeTrackTable(json.tracks));
    expect(state.playlists).toStrictEqual(json.playlists);
  });
});
