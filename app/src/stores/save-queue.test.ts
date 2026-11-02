import { beforeEach, describe, expect, it, vi } from 'vitest';

const hooks = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: hooks.invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('../services/playback.svelte.ts', () => ({
  playbackPause: vi.fn(),
  playbackPlay: vi.fn(async () => undefined),
  playbackSeek: vi.fn(),
  playbackSetPosition: vi.fn(),
  playbackStop: vi.fn(async () => undefined),
  subscribePlaybackError: vi.fn(async () => () => {}),
  subscribePlaybackTick: vi.fn(async () => () => {}),
}));

import { flushPendingSave, library, saveState } from './library.svelte.ts';

Object.assign(globalThis, { window: { __TAURI_INTERNALS__: {} } });

type Held = { resolve: () => void; reject: (e: unknown) => void; names: string[] };

function saves() {
  return hooks.invoke.mock.calls.filter(([command]) => command === 'save_state');
}

function holdSaves() {
  const held: Held[] = [];
  hooks.invoke.mockImplementation((_cmd: string, args: { playlists: { name: string }[] }) => {
    return new Promise<void>((resolve, reject) => {
      held.push({ resolve, reject, names: args.playlists.map((p) => p.name) });
    });
  });
  return held;
}

function setNames(...names: string[]) {
  library.playlists = names.map((name, i) => ({ id: i + 1, name, track_ids: [], parent_id: 0, is_folder: false })) as never;
}

const tick = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  hooks.invoke.mockReset();
  hooks.invoke.mockResolvedValue(undefined);
  setNames('a');
});

describe('saveState queue', () => {
  it('sends the first save immediately', async () => {
    await saveState();
    expect(saves()).toHaveLength(1);
  });

  it('keeps one save in flight and coalesces later calls into one newest snapshot', async () => {
    const held = holdSaves();
    const first = saveState();
    setNames('b');
    const second = saveState();
    setNames('c');
    const third = saveState();
    await tick();
    expect(held).toHaveLength(1);
    expect(held[0].names).toEqual(['a']);

    held[0].resolve();
    await first;
    await tick();
    expect(held).toHaveLength(2);
    expect(held[1].names).toEqual(['c']);

    let secondDone = false;
    void second.then(() => (secondDone = true));
    await tick();
    expect(secondDone).toBe(false);
    held[1].resolve();
    await Promise.all([second, third]);
    expect(held).toHaveLength(2);
  });

  it('writes the newest snapshot last when saves finish slowly', async () => {
    const held = holdSaves();
    const done = [saveState()];
    for (const name of ['b', 'c', 'd']) {
      setNames(name);
      done.push(saveState());
    }
    await tick();
    held[0].resolve();
    await tick();
    held[1].resolve();
    await Promise.all(done);
    expect(held.map((h) => h.names[0])).toEqual(['a', 'd']);
  });

  it('keeps later saves going after a failed save and does not throw', async () => {
    const held = holdSaves();
    const first = saveState();
    setNames('b');
    const second = saveState();
    await tick();
    held[0].reject('disk full');
    await expect(first).resolves.toBeUndefined();
    await tick();
    expect(held).toHaveLength(2);
    held[1].resolve();
    await expect(second).resolves.toBeUndefined();
  });

  it('starts a new save after the queue drained', async () => {
    await saveState();
    await saveState();
    expect(saves()).toHaveLength(2);
  });

  it('sends the waiting snapshot on page hide', async () => {
    const held = holdSaves();
    void saveState();
    setNames('z');
    void saveState();
    await tick();
    flushPendingSave();
    await tick();
    expect(held.map((h) => h.names[0])).toEqual(['a', 'z']);
  });
});
