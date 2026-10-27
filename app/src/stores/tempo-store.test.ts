import { beforeEach, describe, expect, it, vi } from 'vitest';

const engine = vi.hoisted(() => ({
  tempo: vi.fn(async (_ratio: number) => undefined),
  shift: vi.fn(async (_semitones: number) => undefined),
  lock: vi.fn(async (_on: boolean) => undefined),
  ticks: [] as Array<(tick: Record<string, unknown>) => void>,
}));

vi.mock('../services/playback.svelte.ts', () => ({
  playbackPause: vi.fn(),
  playbackPlay: vi.fn(async () => undefined),
  playbackSeek: vi.fn(),
  playbackSetPosition: vi.fn(),
  playbackStop: vi.fn(async () => undefined),
  playbackSetTempo: engine.tempo,
  playbackSetKeyShift: engine.shift,
  playbackSetKeyLock: engine.lock,
  subscribePlaybackError: vi.fn(async () => () => {}),
  subscribePlaybackTick: vi.fn(async (handler: (tick: Record<string, unknown>) => void) => {
    engine.ticks.push(handler);
    return () => {};
  }),
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(async () => undefined) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

import {
  resetTempo,
  setKeyLock,
  setKeyShift,
  setTempoPercent,
  setTempoRange,
  tempo,
  tempoActive,
} from './tempo.svelte.ts';
import { initPlayerTransport, loadPlayerTrack, player } from './player.svelte.ts';

beforeEach(() => {
  resetTempo();
  setTempoRange(8);
  setKeyLock(true);
  vi.clearAllMocks();
});

describe('tempo store', () => {
  it('starts at the track tempo with key lock on', () => {
    expect(tempo.percent).toBe(0);
    expect(tempo.keyShift).toBe(0);
    expect(tempo.keyLock).toBe(true);
    expect(tempo.range).toBe(8);
    expect(tempoActive()).toBe(false);
  });

  it('pushes the ratio to the engine', () => {
    setTempoPercent(4.2);
    expect(engine.tempo).toHaveBeenLastCalledWith(1.042);
    expect(tempo.percent).toBe(4.2);
    expect(tempoActive()).toBe(true);
  });

  it('widens the range for a typed tempo and clamps at 50 %', () => {
    setTempoPercent(12);
    expect(tempo.range).toBe(16);
    setTempoPercent(-80);
    expect(tempo.percent).toBe(-50);
    expect(tempo.range).toBe(50);
    expect(engine.tempo).toHaveBeenLastCalledWith(0.5);
  });

  it('narrowing the range pulls the tempo inside it', () => {
    setTempoPercent(12);
    setTempoRange(8);
    expect(tempo.percent).toBe(8);
    expect(engine.tempo).toHaveBeenLastCalledWith(1.08);
  });

  it('clamps the key shift to an octave', () => {
    setKeyShift(3);
    expect(engine.shift).toHaveBeenLastCalledWith(3);
    setKeyShift(40);
    expect(tempo.keyShift).toBe(12);
    setKeyShift(-40);
    expect(tempo.keyShift).toBe(-12);
  });

  it('toggles key lock in the engine', () => {
    setKeyLock(false);
    expect(tempo.keyLock).toBe(false);
    expect(engine.lock).toHaveBeenLastCalledWith(false);
  });

  it('reset goes back to the track values and tells the engine', () => {
    setTempoPercent(6);
    setKeyShift(2);
    resetTempo();
    expect(tempo.percent).toBe(0);
    expect(tempo.keyShift).toBe(0);
    expect(engine.tempo).toHaveBeenLastCalledWith(1);
    expect(engine.shift).toHaveBeenLastCalledWith(0);
  });

  it('loading a track drops tempo and key shift but keeps range and key lock', async () => {
    setTempoPercent(12);
    setKeyShift(4);
    setKeyLock(false);
    await loadPlayerTrack({ id: 1, filePath: '/music/a.wav', title: 'a', raw: { id: 1, source_path: '/music/a.wav', title: 'a', artist: '', tempo: 12800, key: '8A', duration_secs: 200 } });
    expect(tempo.percent).toBe(0);
    expect(tempo.keyShift).toBe(0);
    expect(tempo.range).toBe(16);
    expect(tempo.keyLock).toBe(false);
  });

  it('the player follows the live ratio the engine reports', async () => {
    await initPlayerTransport();
    await loadPlayerTrack({ id: 2, filePath: '/music/b.wav', title: 'b', raw: { id: 2, source_path: '/music/b.wav', title: 'b', artist: '', tempo: 12800, key: '8A', duration_secs: 200 } });
    engine.ticks.forEach((tick) => tick({ position_ms: 1000, duration_ms: 200000, playing: true, tempo_ratio: 1.08 }));
    expect(tempo.liveRatio).toBe(1.08);
    expect(player.tempo).toBe(tempo);
  });
});
