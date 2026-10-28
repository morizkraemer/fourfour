import { getIsTauri } from '../services/tauri.svelte.ts';
import {
  ANALYSIS_ENGINE_CHOICES,
  appSettings,
  changeBufferFrames,
  changeSampleRate,
  setAnalysisEngine,
  setAutoAnalyzeNew,
  type AnalysisEngineChoice,
} from './app-settings.svelte.ts';
import { BUFFER_FRAME_OPTIONS, SAMPLE_RATE_OPTIONS, playbackSettings } from './playback-settings.svelte.ts';
import { openSettings } from './ui.svelte.ts';
import type { PaletteSettingSource } from '../shortcuts/palette-model.ts';

export const SETTINGS_PAGES = [
  { key: 'library', label: 'Library', keywords: ['rekordbox', 'import', 'folder', 'path', 'database'] },
  { key: 'analysis', label: 'Analysis', keywords: ['bpm', 'key', 'engine', 'auto', 'detect'] },
  { key: 'audio', label: 'Audio', keywords: ['output', 'device', 'sample', 'buffer', 'latency', 'sound'] },
  { key: 'keyboard', label: 'Keyboard', keywords: ['shortcuts', 'keys', 'bindings', 'hotkeys'] },
  { key: 'advanced', label: 'Advanced', keywords: ['reset', 'debug', 'developer'] },
] as const;

export type SettingsPageKey = (typeof SETTINGS_PAGES)[number]['key'];

function numberChoices(options: ReadonlyArray<{ value: number; label: string }>) {
  return options.map((o) => ({ value: String(o.value), label: o.label }));
}

function cycle(options: ReadonlyArray<{ value: string }>, current: string): string {
  const index = options.findIndex((o) => o.value === current);
  return options[(index + 1) % options.length].value;
}

export function buildSettingSources(audioAvailable: boolean = getIsTauri()): PaletteSettingSource[] {
  const engineOptions = [...ANALYSIS_ENGINE_CHOICES];
  const sampleRateOptions = numberChoices(SAMPLE_RATE_OPTIONS);
  const bufferOptions = numberChoices(BUFFER_FRAME_OPTIONS);

  const controls: PaletteSettingSource[] = [
    {
      id: 'analysis.autoAnalyzeNew',
      label: 'Settings: Analyze new tracks',
      keywords: ['auto', 'import', 'automatic'],
      control: { kind: 'switch', on: appSettings.autoAnalyzeNew },
      change: () => void setAutoAnalyzeNew(!appSettings.autoAnalyzeNew),
    },
    {
      id: 'analysis.engine',
      label: 'Settings: Analysis engine',
      control: { kind: 'choice', value: appSettings.analysisEngine, options: engineOptions },
      change: (value) =>
        void setAnalysisEngine((value ?? cycle(engineOptions, appSettings.analysisEngine)) as AnalysisEngineChoice),
    },
  ];

  if (audioAvailable) {
    controls.push(
      {
        id: 'audio.sampleRate',
        label: 'Settings: Sample rate',
        control: { kind: 'choice', value: String(playbackSettings.sampleRate), options: sampleRateOptions },
        change: (value) =>
          void changeSampleRate(Number(value ?? cycle(sampleRateOptions, String(playbackSettings.sampleRate)))),
      },
      {
        id: 'audio.bufferFrames',
        label: 'Settings: Buffer size',
        control: { kind: 'choice', value: String(playbackSettings.bufferFrames), options: bufferOptions },
        change: (value) =>
          void changeBufferFrames(Number(value ?? cycle(bufferOptions, String(playbackSettings.bufferFrames)))),
      },
    );
  }

  const pages: PaletteSettingSource[] = SETTINGS_PAGES.map((page) => ({
    id: `page.${page.key}`,
    label: `Open settings › ${page.label}`,
    keywords: [...page.keywords],
    change: () => openSettings(page.key),
  }));

  return [...controls, ...pages];
}
