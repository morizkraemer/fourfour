/**
 * playback-settings.svelte.ts — Preview playback backend + CoreAudio device params.
 * Persisted in localStorage; routed by playback.svelte.ts transport layer.
 */

import { getIsTauri } from '../services/tauri.svelte.ts';

const ENGINE_KEY = 'fourfour.playbackEngine.v2';
const SAMPLE_RATE_KEY = 'fourfour.playbackSampleRate.v1';
const BUFFER_FRAMES_KEY = 'fourfour.playbackBufferFrames.v1';

export type PlaybackEngineId = 'coreaudio' | 'mock';

export const DEFAULT_SAMPLE_RATE = 48_000;
export const DEFAULT_BUFFER_FRAMES = 128;

export const PLAYBACK_ENGINE_OPTIONS: ReadonlyArray<{
  id: PlaybackEngineId;
  label: string;
  description: string;
}> = [
  {
    id: 'coreaudio',
    label: 'CoreAudio',
    description: 'Native macOS output via cpal (symphonia decoders).',
  },
  {
    id: 'mock',
    label: 'Mock',
    description: 'Simulated transport with no audio output — browser dev and UI testing.',
  },
];

export const SAMPLE_RATE_OPTIONS: ReadonlyArray<{ value: number; label: string }> = [
  { value: 44_100, label: '44.1 kHz' },
  { value: 48_000, label: '48 kHz' },
  { value: 96_000, label: '96 kHz' },
];

export const BUFFER_FRAME_OPTIONS: ReadonlyArray<{ value: number; label: string }> = [
  { value: 64, label: '64' },
  { value: 128, label: '128' },
  { value: 256, label: '256' },
  { value: 512, label: '512' },
  { value: 1024, label: '1024' },
];

function defaultEngine(): PlaybackEngineId {
  return getIsTauri() ? 'coreaudio' : 'mock';
}

function loadEngine(): PlaybackEngineId {
  try {
    const raw = localStorage.getItem(ENGINE_KEY);
    if (raw === 'coreaudio' || raw === 'mock') return raw;
    const legacy = localStorage.getItem('fourfour.playbackEngine.v1');
    if (legacy === 'rodio') return 'coreaudio';
    if (legacy === 'mock') return 'mock';
  } catch {
    /* private mode / unavailable */
  }
  return defaultEngine();
}

function loadSampleRate(): number {
  try {
    const raw = localStorage.getItem(SAMPLE_RATE_KEY);
    if (raw) {
      const n = Number(raw);
      if (SAMPLE_RATE_OPTIONS.some((o) => o.value === n)) return n;
    }
  } catch {
    /* ignore */
  }
  return DEFAULT_SAMPLE_RATE;
}

function loadBufferFrames(): number {
  try {
    const raw = localStorage.getItem(BUFFER_FRAMES_KEY);
    if (raw) {
      const n = Number(raw);
      if (BUFFER_FRAME_OPTIONS.some((o) => o.value === n)) return n;
    }
  } catch {
    /* ignore */
  }
  return DEFAULT_BUFFER_FRAMES;
}

function persistEngine(id: PlaybackEngineId) {
  try {
    localStorage.setItem(ENGINE_KEY, id);
  } catch {
    /* ignore */
  }
}

function persistSampleRate(rate: number) {
  try {
    localStorage.setItem(SAMPLE_RATE_KEY, String(rate));
  } catch {
    /* ignore */
  }
}

function persistBufferFrames(frames: number) {
  try {
    localStorage.setItem(BUFFER_FRAMES_KEY, String(frames));
  } catch {
    /* ignore */
  }
}

export const playbackSettings = $state({
  engine: loadEngine() as PlaybackEngineId,
  sampleRate: loadSampleRate(),
  bufferFrames: loadBufferFrames(),
});

/** Engines the user can pick in the current runtime. */
export function availablePlaybackEngines(): PlaybackEngineId[] {
  return getIsTauri() ? ['coreaudio', 'mock'] : ['mock'];
}

export function playbackEngineLabel(id: PlaybackEngineId): string {
  return PLAYBACK_ENGINE_OPTIONS.find((o) => o.id === id)?.label ?? id;
}

/** True when transport should use the mock timer instead of Tauri invoke. */
export function useMockPlayback(): boolean {
  if (!getIsTauri()) return true;
  return playbackSettings.engine === 'mock';
}

export function setPlaybackEngine(id: PlaybackEngineId): void {
  const allowed = availablePlaybackEngines();
  if (!allowed.includes(id)) return;
  if (playbackSettings.engine === id) return;
  playbackSettings.engine = id;
  persistEngine(id);
}

export function setPlaybackSampleRate(rate: number): void {
  if (!SAMPLE_RATE_OPTIONS.some((o) => o.value === rate)) return;
  if (playbackSettings.sampleRate === rate) return;
  playbackSettings.sampleRate = rate;
  persistSampleRate(rate);
}

export function setPlaybackBufferFrames(frames: number): void {
  if (!BUFFER_FRAME_OPTIONS.some((o) => o.value === frames)) return;
  if (playbackSettings.bufferFrames === frames) return;
  playbackSettings.bufferFrames = frames;
  persistBufferFrames(frames);
}
