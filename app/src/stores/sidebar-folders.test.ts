import { beforeEach, describe, expect, it, vi } from 'vitest';

const KEY = 'ff.sidebar.collapsedFolders';
const storage = new Map<string, string>();

function stubStorage(overrides: Partial<Storage> = {}) {
  vi.stubGlobal('localStorage', {
    getItem: (key: string) => storage.get(key) ?? null,
    setItem: (key: string, value: string) => void storage.set(key, value),
    ...overrides,
  });
}

async function freshStore() {
  vi.resetModules();
  return import('./sidebar-folders.svelte.ts');
}

beforeEach(() => {
  storage.clear();
  stubStorage();
});

describe('collapsed sidebar folders', () => {
  it('toggles a folder and remembers it across a reload', async () => {
    const first = await freshStore();
    first.toggleFolder(3);
    first.toggleFolder(9);
    first.toggleFolder(3);
    expect(first.folderState.collapsed).toEqual([9]);

    const reloaded = await freshStore();
    expect(reloaded.folderState.collapsed).toEqual([9]);
  });

  it('expands only a collapsed folder and saves only then', async () => {
    storage.set(KEY, '[4,5]');
    const store = await freshStore();
    const setItem = vi.fn((key: string, value: string) => void storage.set(key, value));
    stubStorage({ setItem });

    store.expandFolder(7);
    expect(setItem).not.toHaveBeenCalled();
    store.expandFolder(4);
    expect(store.folderState.collapsed).toEqual([5]);
    expect(storage.get(KEY)).toBe('[5]');
  });

  it('starts empty from damaged or foreign stored data', async () => {
    storage.set(KEY, '{not json');
    expect((await freshStore()).folderState.collapsed).toEqual([]);
    storage.set(KEY, '{"a":1}');
    expect((await freshStore()).folderState.collapsed).toEqual([]);
    storage.set(KEY, '[1,"x",null,2]');
    expect((await freshStore()).folderState.collapsed).toEqual([1, 2]);
  });

  it('still works when storage refuses to save', async () => {
    const store = await freshStore();
    stubStorage({
      setItem: () => {
        throw new Error('quota');
      },
    });
    store.toggleFolder(2);
    expect(store.folderState.collapsed).toEqual([2]);
  });
});
