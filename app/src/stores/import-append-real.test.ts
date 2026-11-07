import { readFileSync } from 'node:fs';
import { describe, expect, it, vi } from 'vitest';

const env = (globalThis as unknown as { process: { env: Record<string, string | undefined> } }).process.env;
const STATE = env.FF_PERF_STATE;

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
import { decodeTrackTable, isTrackTable } from '../services/track-table.ts';
import { type AppendOp, appendScenario, createHarness, lcg } from './import-append.ts';

const WORDS = ['house', 'dub', 'night', 'zeta', 'Ärger', 'lagos', 'ghosts'];

describe.skipIf(!STATE)('rows landed by an import on a real payload', () => {
  it('give the same search text and the same sorted orders as the library loaded whole', async () => {
    const dumped = JSON.parse(readFileSync(STATE!, 'utf8'));
    const initial = (isTrackTable(dumped.tracks) ? decodeTrackTable(dumped.tracks) : dumped.tracks) as TrackInfo[];
    const random = lcg(144);
    let nextId = Math.max(...initial.map((t) => t.id)) + 1;
    const batch = (size: number): TrackInfo[] =>
      Array.from({ length: size }, () => {
        const from = initial[Math.floor(random() * initial.length)];
        const id = nextId++;
        const word = () => WORDS[Math.floor(random() * WORDS.length)];
        return {
          ...from,
          id,
          source_path: `/Downloads/Crate/F${id % 10}/Track ${id}.mp3`,
          title: random() < 0.5 ? `${word()} ${id}` : from.title,
          artist: random() < 0.3 ? word() : from.artist,
          tempo: random() < 0.3 ? 0 : from.tempo,
          has_analysis: false,
        };
      });
    const pickIds = (count: number) => Array.from({ length: count }, () => initial[Math.floor(random() * initial.length)].id);
    const ops: AppendOp[] = [
      { kind: 'append', batch: batch(1000) },
      { kind: 'check' },
      { kind: 'bump', ids: pickIds(30), seed: 1 },
      { kind: 'append', batch: batch(1) },
      { kind: 'append', batch: batch(250) },
      { kind: 'bump', ids: pickIds(900), seed: 2 },
      { kind: 'check' },
    ];
    const result = await appendScenario(createHarness(hooks), initial, ops, true);
    expect(result).toEqual({ checks: 2, mismatches: [] });
  }, 1_800_000);
});
