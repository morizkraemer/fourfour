import { describe, expect, it, vi } from 'vitest';

const hooks = vi.hoisted(() => ({
  state: { tracks: [] as unknown[], playlists: [] as unknown[] },
  added: [] as unknown[],
}));

vi.mock('../services/playback.svelte.ts', () => ({
  playbackPause: vi.fn(),
  playbackPlay: vi.fn(async () => undefined),
  playbackSeek: vi.fn(),
  playbackSetPosition: vi.fn(),
  playbackStop: vi.fn(async () => undefined),
  subscribePlaybackError: vi.fn(async () => () => {}),
  subscribePlaybackTick: vi.fn(async () => () => {}),
}));
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (command: string) => {
    if (command === 'load_state') return structuredClone(hooks.state);
    if (command === 'plan_import') {
      return { token: 1, new_count: hooks.added.length, already_in_library: 0, duplicates: [], skipped: { unsupported: 0, unsupported_exts: [], unreadable: 0 } };
    }
    if (command === 'commit_import') return { added: structuredClone(hooks.added), skipped_ids: [] };
    return undefined;
  }),
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import type { TrackInfo } from '../services/tauri.svelte.ts';
import { type AppendOp, appendScenario, createHarness, lcg, randomTrackInfo } from './import-append.ts';

const harness = createHarness(hooks);

function script(seed: number): { initial: TrackInfo[]; ops: AppendOp[] } {
  const random = lcg(seed);
  const initial = Array.from({ length: Math.floor(random() * 120) }, (_, i) => randomTrackInfo(random, i + 1, '/m'));
  let nextId = initial.length + 1;
  let count = initial.length;
  const ops: AppendOp[] = [];
  const steps = 3 + Math.floor(random() * 6);
  for (let s = 0; s < steps; s++) {
    const pick = random();
    if (pick < 0.45) {
      const size = 1 + Math.floor(random() * (random() < 0.2 ? 200 : 25));
      ops.push({ kind: 'append', batch: Array.from({ length: size }, () => randomTrackInfo(random, nextId++, `/Downloads/Crate/F${Math.floor(random() * 3)}`)) });
      count += size;
    } else if (pick < 0.8 && count > 0) {
      const size = 1 + Math.floor(random() * (random() < 0.15 ? 60 : 4));
      ops.push({ kind: 'bump', ids: Array.from({ length: size }, () => 1 + Math.floor(random() * count)), seed: Math.floor(random() * 1000) });
    } else {
      ops.push({ kind: 'check' });
    }
  }
  ops.push({ kind: 'check' });
  return { initial, ops };
}

describe('rows landed by an import', () => {
  it('give the same search text and the same sorted orders as a library loaded whole', async () => {
    let checks = 0;
    for (let seed = 1; seed <= 40; seed++) {
      const { initial, ops } = script(seed);
      const result = await appendScenario(harness, initial, ops, seed % 4 !== 0);
      expect({ seed, mismatches: result.mismatches }).toEqual({ seed, mismatches: [] });
      checks += result.checks;
    }
    expect(checks).toBeGreaterThan(60);
  }, 600_000);
});
