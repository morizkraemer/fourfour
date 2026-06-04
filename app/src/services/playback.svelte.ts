/**
 * playback.svelte.ts — Tauri playback transport (rodio backend).
 */

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getIsTauri } from './tauri.svelte.ts';

export interface PlaybackTick {
  position_ms: number;
  duration_ms: number;
  playing: boolean;
}

let mockPlaying = false;
let mockPositionMs = 0;
let mockDurationMs = 0;
let mockTimer: ReturnType<typeof setInterval> | null = null;
const mockListeners = new Set<(tick: PlaybackTick) => void>();

function emitMock() {
  const tick: PlaybackTick = {
    position_ms: mockPositionMs,
    duration_ms: mockDurationMs,
    playing: mockPlaying,
  };
  mockListeners.forEach((fn) => fn(tick));
}

function startMockTimer() {
  if (mockTimer) return;
  mockTimer = setInterval(() => {
    if (!mockPlaying || mockDurationMs <= 0) return;
    mockPositionMs = Math.min(mockDurationMs, mockPositionMs + 50);
    if (mockPositionMs >= mockDurationMs) mockPlaying = false;
    emitMock();
  }, 50);
}

export async function playbackPlay(path: string, positionMs = 0): Promise<void> {
  if (!getIsTauri()) {
    mockDurationMs = 240_000;
    mockPositionMs = positionMs;
    mockPlaying = true;
    startMockTimer();
    emitMock();
    return;
  }
  await invoke('playback_play', { path, positionMs });
}

export async function playbackPause(): Promise<void> {
  if (!getIsTauri()) {
    mockPlaying = false;
    emitMock();
    return;
  }
  await invoke('playback_pause');
}

export async function playbackResume(): Promise<void> {
  if (!getIsTauri()) {
    mockPlaying = true;
    startMockTimer();
    emitMock();
    return;
  }
  await invoke('playback_resume');
}

export async function playbackSeek(positionMs: number): Promise<void> {
  if (!getIsTauri()) {
    mockPositionMs = positionMs;
    emitMock();
    return;
  }
  await invoke('playback_seek', { positionMs });
}

export async function playbackStop(): Promise<void> {
  if (!getIsTauri()) {
    mockPlaying = false;
    mockPositionMs = 0;
    if (mockTimer) {
      clearInterval(mockTimer);
      mockTimer = null;
    }
    emitMock();
    return;
  }
  await invoke('playback_stop');
}

export async function subscribePlaybackTick(
  handler: (tick: PlaybackTick) => void,
): Promise<() => void> {
  if (!getIsTauri()) {
    mockListeners.add(handler);
    return () => mockListeners.delete(handler);
  }
  const unlisten = await listen<PlaybackTick>('playback-tick', (event) => {
    handler(event.payload);
  });
  return unlisten;
}
