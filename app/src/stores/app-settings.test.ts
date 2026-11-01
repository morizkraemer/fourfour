import { beforeEach, describe, expect, it, vi } from 'vitest';

const hooks = vi.hoisted(() => ({
  getAutoAnalyzeNew: vi.fn(),
  setAutoAnalyzeNew: vi.fn(),
  getAnalysisBackend: vi.fn(),
  setAnalysisBackend: vi.fn(),
  getAnalyzeAfterImport: vi.fn(),
  setAnalyzeAfterImport: vi.fn(),
  playbackStop: vi.fn(),
  syncPlaybackAudioConfig: vi.fn(),
  tauri: { on: true },
}));

vi.mock('../services/tauri.svelte.ts', () => ({
  getAutoAnalyzeNew: hooks.getAutoAnalyzeNew,
  setAutoAnalyzeNew: hooks.setAutoAnalyzeNew,
  getAnalysisBackend: hooks.getAnalysisBackend,
  setAnalysisBackend: hooks.setAnalysisBackend,
  getAnalyzeAfterImport: hooks.getAnalyzeAfterImport,
  setAnalyzeAfterImport: hooks.setAnalyzeAfterImport,
  getIsTauri: () => hooks.tauri.on,
}));
vi.mock('../services/playback.svelte.ts', () => ({
  playbackStop: hooks.playbackStop,
  syncPlaybackAudioConfig: hooks.syncPlaybackAudioConfig,
}));

const storage = new Map<string, string>();
vi.stubGlobal('localStorage', {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => void storage.set(key, value),
  removeItem: (key: string) => void storage.delete(key),
});

import {
  appSettings,
  changeBufferFrames,
  changeOutputDevice,
  changeSampleRate,
  loadAppSettings,
  setAnalysisEngine,
  setAnalyzeAfterImport,
  setAutoAnalyzeNew,
} from './app-settings.svelte.ts';
import { playbackSettings } from './playback-settings.svelte.ts';

beforeEach(() => {
  for (const mock of Object.values(hooks)) if (typeof mock === 'function') mock.mockReset();
  hooks.tauri.on = true;
  hooks.setAutoAnalyzeNew.mockResolvedValue(undefined);
  hooks.setAnalyzeAfterImport.mockResolvedValue(undefined);
  hooks.setAnalysisBackend.mockResolvedValue(undefined);
  hooks.playbackStop.mockResolvedValue(undefined);
  hooks.syncPlaybackAudioConfig.mockResolvedValue(undefined);
  storage.clear();
  appSettings.autoAnalyzeNew = true;
  appSettings.analyzeAfterImport = false;
  appSettings.analysisEngine = 'auto';
});

describe('loadAppSettings', () => {
  it('reads the stored values once and marks the settings loaded', async () => {
    hooks.getAutoAnalyzeNew.mockResolvedValue(false);
    hooks.getAnalyzeAfterImport.mockResolvedValue(true);
    hooks.getAnalysisBackend.mockResolvedValue('python');

    await loadAppSettings();
    await loadAppSettings();

    expect(hooks.getAutoAnalyzeNew).toHaveBeenCalledTimes(1);
    expect([appSettings.autoAnalyzeNew, appSettings.analyzeAfterImport, appSettings.analysisEngine, appSettings.loaded]).toEqual([
      false,
      true,
      'python',
      true,
    ]);
  });
});

describe('saving a setting', () => {
  it('shows the new value and keeps it once the save worked', async () => {
    await setAutoAnalyzeNew(false);
    await setAnalyzeAfterImport(true);
    await setAnalysisEngine('rust');

    expect([appSettings.autoAnalyzeNew, appSettings.analyzeAfterImport, appSettings.analysisEngine]).toEqual([false, true, 'rust']);
    expect(hooks.setAnalysisBackend).toHaveBeenCalledWith('rust');
  });

  it('stores automatic as no backend choice', async () => {
    appSettings.analysisEngine = 'python';
    await setAnalysisEngine('auto');
    expect(hooks.setAnalysisBackend).toHaveBeenCalledWith(null);
  });

  it('puts the previous value back and rethrows when the save fails', async () => {
    hooks.setAutoAnalyzeNew.mockRejectedValue(new Error('disk full'));
    hooks.setAnalyzeAfterImport.mockRejectedValue(new Error('disk full'));
    hooks.setAnalysisBackend.mockRejectedValue(new Error('disk full'));

    await expect(setAutoAnalyzeNew(false)).rejects.toThrow('disk full');
    await expect(setAnalyzeAfterImport(true)).rejects.toThrow('disk full');
    await expect(setAnalysisEngine('python')).rejects.toThrow('disk full');

    expect([appSettings.autoAnalyzeNew, appSettings.analyzeAfterImport, appSettings.analysisEngine]).toEqual([true, false, 'auto']);
  });
});

describe('audio device settings', () => {
  it('stops playback, stores the choice and tells the engine', async () => {
    await changeSampleRate(96_000);
    await changeBufferFrames(512);
    await changeOutputDevice('Scarlett');

    expect(hooks.playbackStop).toHaveBeenCalledTimes(3);
    expect(hooks.syncPlaybackAudioConfig).toHaveBeenCalledTimes(3);
    expect([playbackSettings.sampleRate, playbackSettings.bufferFrames, playbackSettings.outputDevice]).toEqual([96_000, 512, 'Scarlett']);
    expect(storage.get('fourfour.playbackSampleRate.v1')).toBe('96000');
    expect(storage.get('fourfour.playbackBufferFrames.v1')).toBe('512');
    expect(storage.get('fourfour.playbackOutputDevice.v1')).toBe('Scarlett');
  });

  it('does nothing when the value is already set', async () => {
    await changeSampleRate(playbackSettings.sampleRate);
    expect(hooks.playbackStop).not.toHaveBeenCalled();
  });

  it('ignores a sample rate or buffer size the settings do not offer', async () => {
    const before = [playbackSettings.sampleRate, playbackSettings.bufferFrames];
    await changeSampleRate(12_345);
    await changeBufferFrames(100);
    expect([playbackSettings.sampleRate, playbackSettings.bufferFrames]).toEqual(before);
    expect(storage.size).toBe(0);
  });

  it('goes back to the system output by removing the stored device', async () => {
    await changeOutputDevice('Scarlett');
    await changeOutputDevice(null);
    expect(playbackSettings.outputDevice).toBeNull();
    expect(storage.has('fourfour.playbackOutputDevice.v1')).toBe(false);
  });

  it('does not touch the engine outside the desktop app', async () => {
    hooks.tauri.on = false;
    await changeBufferFrames(256);
    expect(hooks.syncPlaybackAudioConfig).not.toHaveBeenCalled();
    expect(playbackSettings.bufferFrames).toBe(256);
  });
});
