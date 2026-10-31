import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  BOOT_SNAPSHOT_KEY,
  BOOT_SNAPSHOT_MAX_ROWS,
  bootRowOf,
  bootRowTrack,
  bootSnapshotOf,
  clearBootSnapshot,
  readBootSnapshot,
  writeBootSnapshot,
} from './boot-snapshot.ts';

function memoryStorage() {
  const items = new Map<string, string>();
  return {
    getItem: (key: string) => items.get(key) ?? null,
    setItem: (key: string, value: string) => void items.set(key, value),
    removeItem: (key: string) => void items.delete(key),
    items,
  };
}

function track(id: number, extra: Record<string, unknown> = {}) {
  return {
    id,
    index: String(id).padStart(2, '0'),
    title: `Track ${id}`,
    artist: 'Artist',
    bpm: '124.0',
    key: '8A',
    fav: undefined,
    rating: 3,
    color: null,
    analyzed: true,
    raw: { id, source_path: '/x.mp3' },
    get cover() {
      throw new Error('cover getter must not run');
    },
    get status() {
      throw new Error('status getter must not run');
    },
    get bands() {
      throw new Error('bands getter must not run');
    },
    ...extra,
  };
}

let storage: ReturnType<typeof memoryStorage>;

beforeEach(() => {
  storage = memoryStorage();
  vi.stubGlobal('localStorage', storage);
});
afterEach(() => vi.unstubAllGlobals());

describe('boot snapshot rows', () => {
  it('keeps the plain display cells and skips getters, raw and undefined', () => {
    const row = bootRowOf(track(7));
    expect(row.cells).toEqual({ id: 7, index: '07', title: 'Track 7', artist: 'Artist', bpm: '124.0', key: '8A', rating: 3, color: null, analyzed: true });
    expect(row.wave).toBeUndefined();
  });

  it('carries band previews from bytes or the wire and mono previews', () => {
    const bands = new Uint8Array(384).map((_, i) => i % 120);
    const fromBytes = bootRowOf(track(1, { bandsPreview: bands }));
    const fromWire = bootRowOf(track(2, { bandsWire: fromBytes.wave!.data }));
    const mono = bootRowOf(track(3, { preview: new Uint8Array(400).fill(20) }));
    expect(fromBytes.wave?.kind).toBe('bands');
    expect(fromWire.wave).toEqual(fromBytes.wave);
    expect(mono.wave?.kind).toBe('mono');
    expect(bootRowTrack(fromBytes).bands).not.toBeNull();
    expect(bootRowTrack(mono).bands).not.toBeNull();
    expect(bootRowTrack(bootRowOf(track(4))).bands).toBeNull();
  });

  it('keeps only the first rows of the view, in view order', () => {
    const tracks = Array.from({ length: 500 }, (_, i) => track(500 - i));
    const snapshot = bootSnapshotOf('/lib.db', 'library|title|asc', tracks, 40);
    expect(snapshot.total).toBe(500);
    expect(snapshot.rows.map((r) => r.cells.id)).toEqual(tracks.slice(0, 40).map((t) => t.id));
    expect(bootSnapshotOf('/lib.db', 'v', tracks, 10_000).rows).toHaveLength(BOOT_SNAPSHOT_MAX_ROWS);
  });
});

describe('boot snapshot storage', () => {
  it('reads back only for the same library', () => {
    writeBootSnapshot(bootSnapshotOf('/a/library.db', 'v', [track(1)], 30));
    expect(readBootSnapshot('/a/library.db')?.rows).toHaveLength(1);
    expect(readBootSnapshot('/b/library.db')).toBeNull();
  });

  it('removes the snapshot when the view is empty', () => {
    writeBootSnapshot(bootSnapshotOf('/a/library.db', 'v', [track(1)], 30));
    writeBootSnapshot(bootSnapshotOf('/a/library.db', 'v', [], 30));
    expect(storage.items.has(BOOT_SNAPSHOT_KEY)).toBe(false);
    expect(readBootSnapshot('/a/library.db')).toBeNull();
  });

  it('ignores damaged or foreign data and blocked storage', () => {
    storage.setItem(BOOT_SNAPSHOT_KEY, '{not json');
    expect(readBootSnapshot('/a')).toBeNull();
    storage.setItem(BOOT_SNAPSHOT_KEY, JSON.stringify({ libraryPath: '/a', rows: 'x' }));
    expect(readBootSnapshot('/a')).toBeNull();
    const blocked = () => {
      throw new Error('blocked');
    };
    vi.stubGlobal('localStorage', { getItem: blocked, setItem: blocked, removeItem: blocked });
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    expect(readBootSnapshot('/a')).toBeNull();
    expect(() => writeBootSnapshot(bootSnapshotOf('/a', 'v', [track(1)], 5))).not.toThrow();
    expect(() => clearBootSnapshot()).not.toThrow();
    warn.mockRestore();
  });
});
