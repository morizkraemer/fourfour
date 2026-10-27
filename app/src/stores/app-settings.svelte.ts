import {
  getAutoAnalyzeNew,
  setAutoAnalyzeNew as saveAutoAnalyzeNew,
  getAnalysisBackend,
  setAnalysisBackend as saveAnalysisBackend,
  getIsTauri,
} from '../services/tauri.svelte.ts';
import {
  playbackSettings,
  setPlaybackSampleRate,
  setPlaybackBufferFrames,
  setPlaybackOutputDevice,
} from './playback-settings.svelte.ts';
import { playbackStop, syncPlaybackAudioConfig } from '../services/playback.svelte.ts';

export type AnalysisEngineChoice = 'auto' | 'rust' | 'python';

export const ANALYSIS_ENGINE_CHOICES: ReadonlyArray<{ value: AnalysisEngineChoice; label: string }> = [
  { value: 'auto', label: 'Automatic' },
  { value: 'rust', label: 'Built-in' },
  { value: 'python', label: 'Python' },
];

export const appSettings = $state({
  autoAnalyzeNew: true,
  analysisEngine: 'auto' as AnalysisEngineChoice,
  loaded: false,
});

let loading: Promise<void> | null = null;

function engineChoice(stored: string | null): AnalysisEngineChoice {
  return stored === 'rust' || stored === 'python' ? stored : 'auto';
}

export function loadAppSettings(): Promise<void> {
  loading ??= (async () => {
    const [autoAnalyzeNew, backend] = await Promise.all([
      getAutoAnalyzeNew().catch(() => true),
      getAnalysisBackend().catch(() => null),
    ]);
    appSettings.autoAnalyzeNew = autoAnalyzeNew;
    appSettings.analysisEngine = engineChoice(backend);
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

export async function setAnalysisEngine(choice: AnalysisEngineChoice): Promise<void> {
  const previous = appSettings.analysisEngine;
  appSettings.analysisEngine = choice;
  try {
    await saveAnalysisBackend(choice === 'auto' ? null : choice);
  } catch (err) {
    appSettings.analysisEngine = previous;
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
