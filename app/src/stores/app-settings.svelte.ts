import {
  getAutoAnalyzeNew,
  setAutoAnalyzeNew as saveAutoAnalyzeNew,
  getAnalyzeAfterImport,
  setAnalyzeAfterImport as saveAnalyzeAfterImport,
  getIsTauri,
} from '../services/tauri.svelte.ts';
import {
  playbackSettings,
  setPlaybackSampleRate,
  setPlaybackBufferFrames,
  setPlaybackOutputDevice,
} from './playback-settings.svelte.ts';
import { playbackStop, syncPlaybackAudioConfig } from '../services/playback.svelte.ts';

export const appSettings = $state({
  autoAnalyzeNew: true,
  analyzeAfterImport: false,
  loaded: false,
});

let loading: Promise<void> | null = null;

export function loadAppSettings(): Promise<void> {
  loading ??= (async () => {
    const [autoAnalyzeNew, analyzeAfterImport] = await Promise.all([
      getAutoAnalyzeNew().catch(() => true),
      getAnalyzeAfterImport().catch(() => false),
    ]);
    appSettings.autoAnalyzeNew = autoAnalyzeNew;
    appSettings.analyzeAfterImport = analyzeAfterImport;
    appSettings.loaded = true;
  })();
  return loading;
}

export async function setAutoAnalyzeNew(enabled: boolean): Promise<void> {
  const previous = appSettings.autoAnalyzeNew;
  appSettings.autoAnalyzeNew = enabled;
  try {
    await saveAutoAnalyzeNew(enabled);
  } catch (err) {
    appSettings.autoAnalyzeNew = previous;
    throw err;
  }
}

export async function setAnalyzeAfterImport(enabled: boolean): Promise<void> {
  const previous = appSettings.analyzeAfterImport;
  appSettings.analyzeAfterImport = enabled;
  try {
    await saveAnalyzeAfterImport(enabled);
  } catch (err) {
    appSettings.analyzeAfterImport = previous;
    throw err;
  }
}

async function restartAudio(change: () => void): Promise<void> {
  await playbackStop();
  change();
  if (getIsTauri()) await syncPlaybackAudioConfig();
}

export function changeSampleRate(rate: number): Promise<void> {
  if (rate === playbackSettings.sampleRate) return Promise.resolve();
  return restartAudio(() => setPlaybackSampleRate(rate));
}

export function changeBufferFrames(frames: number): Promise<void> {
  if (frames === playbackSettings.bufferFrames) return Promise.resolve();
  return restartAudio(() => setPlaybackBufferFrames(frames));
}

export function changeOutputDevice(device: string | null): Promise<void> {
  if (device === playbackSettings.outputDevice) return Promise.resolve();
  return restartAudio(() => setPlaybackOutputDevice(device));
}
