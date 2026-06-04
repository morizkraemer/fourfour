/**
 * playback.svelte.ts — Tauri playback transport (CoreAudio / cpal backend).
 */

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getIsTauri } from './tauri.svelte.ts';
import {
  playbackSettings,
  useMockPlayback,
} from '../stores/playback-settings.svelte.ts';

export interface PlaybackAudioConfig {
  sample_rate: number;
  buffer_frames: number;
}

export interface PlaybackTick {
  position_ms: number;
  duration_ms: number;
  playing: boolean;
  level_left?: number;
  level_right?: number;
}

let mockPlaying = false;
let mockPositionMs = 0;
let mockDurationMs = 0;
let mockLevelLeft = 0;
let mockLevelRight = 0;
let mockLevelPhase = 0;
let mockTimer: ReturnType<typeof setInterval> | null = null;
const mockListeners = new Set<(tick: PlaybackTick) => void>();

const MOCK_LEVEL_DECAY = 0.82;
const MOCK_LEVEL_ATTACK = 0.35;

function updateMockLevels() {
  if (!mockPlaying) {
    mockLevelLeft *= MOCK_LEVEL_DECAY;
    mockLevelRight *= MOCK_LEVEL_DECAY;
    if (mockLevelLeft < 0.01) mockLevelLeft = 0;
    if (mockLevelRight < 0.01) mockLevelRight = 0;
    return;
  }
  mockLevelPhase += 0.22;
  const base = 0.38 + 0.28 * Math.sin(mockLevelPhase);
  const targetL = Math.min(1, base + 0.12 * Math.sin(mockLevelPhase * 2.1));
  const targetR = Math.min(1, base * 0.92 + 0.1 * Math.cos(mockLevelPhase * 1.6));
  mockLevelLeft = Math.min(1, mockLevelLeft * MOCK_LEVEL_DECAY + targetL * MOCK_LEVEL_ATTACK);
  mockLevelRight = Math.min(1, mockLevelRight * MOCK_LEVEL_DECAY + targetR * MOCK_LEVEL_ATTACK);
}

function emitMock() {
  updateMockLevels();
  const tick: PlaybackTick = {
    position_ms: mockPositionMs,
    duration_ms: mockDurationMs,
    playing: mockPlaying,
    level_left: mockLevelLeft,
    level_right: mockLevelRight,
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

function useMock(): boolean {
  return useMockPlayback();
}

/** Apply persisted sample rate / buffer size to the native output device. */
export async function syncPlaybackAudioConfig(): Promise<PlaybackAudioConfig | null> {
  if (!getIsTauri() || useMock()) return null;
  const config = await invoke<PlaybackAudioConfig>('playback_configure', {
    sampleRate: playbackSettings.sampleRate,
    bufferFrames: playbackSettings.bufferFrames,
  });
  return config;
}

export async function getPlaybackAudioConfig(): Promise<PlaybackAudioConfig | null> {
  if (!getIsTauri()) return null;
  return invoke<PlaybackAudioConfig>('playback_get_config');
}

export async function playbackPlay(
  path: string,
  positionMs = 0,
  durationMs?: number,
): Promise<void> {
  if (useMock()) {
    mockDurationMs = durationMs && durationMs > 0 ? durationMs : 240_000;
    mockPositionMs = positionMs;
    mockPlaying = true;
    startMockTimer();
    emitMock();
    return;
  }
  await invoke('playback_play', {
    path,
    positionMs,
    durationMs: durationMs && durationMs > 0 ? durationMs : null,
  });
}

export async function playbackPause(): Promise<void> {
  if (useMock()) {
    mockPlaying = false;
    emitMock();
    return;
  }
  await invoke('playback_pause');
}

export async function playbackResume(): Promise<void> {
  if (useMock()) {
    mockPlaying = true;
    startMockTimer();
    emitMock();
    return;
  }
  await invoke('playback_resume');
}

export async function playbackSeek(positionMs: number): Promise<void> {
  if (useMock()) {
    mockPositionMs = positionMs;
    emitMock();
    return;
  }
  await invoke('playback_seek', { positionMs });
}

export async function playbackStop(): Promise<void> {
  if (useMock()) {
    mockPlaying = false;
    mockPositionMs = 0;
    mockLevelLeft = 0;
    mockLevelRight = 0;
    mockLevelPhase = 0;
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
  if (useMock()) {
    mockListeners.add(handler);
    return () => mockListeners.delete(handler);
  }
  const unlisten = await listen<PlaybackTick>('playback-tick', (event) => {
    handler(event.payload);
  });
  return unlisten;
}
