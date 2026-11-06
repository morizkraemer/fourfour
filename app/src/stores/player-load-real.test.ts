import { createHash } from 'node:crypto';
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { describe, expect, it, vi } from 'vitest';

const dumpDir = (globalThis as unknown as { process: { env: Record<string, string | undefined> } }).process.env.FF_R128_DUMP_DIR;
const outFile = (globalThis as unknown as { process: { env: Record<string, string | undefined> } }).process.env.FF_R128_HASH_OUT;

vi.mock('../services/playback.svelte.ts', () => ({
  playbackPause: vi.fn(),
  playbackPlay: vi.fn(async () => undefined),
  playbackSeek: vi.fn(),
  playbackSetPosition: vi.fn(),
  playbackStop: vi.fn(async () => undefined),
  subscribePlaybackError: vi.fn(async () => () => {}),
  subscribePlaybackTick: vi.fn(async () => () => {}),
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(async () => undefined) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import { invalidateAnalysisCache } from '../services/tauri.svelte.ts';
import { loadPlayerTrack, player } from './player.svelte.ts';

const invokeMock = vi.mocked(invoke);

function feed(bytes: Uint8Array): ArrayBuffer {
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
}

function digest(value: unknown): string {
  const hash = createHash('sha256');
  const walk = (v: unknown) => {
    if (ArrayBuffer.isView(v)) {
      hash.update(`${v.constructor.name}:${v.byteLength}:`);
      hash.update(new Uint8Array(v.buffer, v.byteOffset, v.byteLength));
    } else if (Array.isArray(v)) {
      hash.update(`[${v.length}`);
      v.forEach(walk);
    } else if (v && typeof v === 'object') {
      const keys = Object.keys(v);
      hash.update(`{${keys.join(',')}`);
      for (const key of keys) walk((v as Record<string, unknown>)[key]);
    } else {
      hash.update(`${typeof v}:${String(v)};`);
    }
  };
  walk(value);
  return hash.digest('hex');
}

describe.skipIf(!dumpDir)('the player track built from real get_analysis_data answers', () => {
  it('hashes every field the player receives', async () => {
    const files = readdirSync(dumpDir!).filter((f: string) => f.endsWith('.analysis')).length;
    const lines: string[] = [];
    for (let k = 0; k < files; k++) {
      const analysis = readFileSync(`${dumpDir}/${k}.analysis`);
      const detail = feed(readFileSync(`${dumpDir}/${k}.detail`));
      invokeMock.mockReset();
      invokeMock.mockImplementation(async (command: string): Promise<any> => {
        if (command === 'get_analysis_data') return analysis[0] === 0x7b ? JSON.parse(new TextDecoder().decode(analysis)) : feed(analysis);
        if (command === 'get_waveform_detail') return detail.slice(0);
        return undefined;
      });
      invalidateAnalysisCache();
      const id = k + 1;
      await loadPlayerTrack({ id, source_path: `/music/${id}.flac`, title: `T${id}`, artist: 'A', tempo: 12000, key: '1A', duration_secs: 300, has_analysis: true });
      await vi.waitFor(() => expect(player.track?.bandsZoom).not.toBeNull(), { timeout: 5000 });
      await vi.waitFor(() => expect(player.loading).toBe(false), { timeout: 5000 });
      const { bandsZoom, ...analysed } = player.track!;
      lines.push(`${id} ${digest(analysed)} ${digest(bandsZoom)}`);
    }
    if (outFile) writeFileSync(outFile, `${lines.join('\n')}\n`);
    expect(lines.length).toBe(files);
  });
});
