import { beforeEach, describe, expect, it } from 'vitest';
import { appSettings, loadAppSettings, setAutoAnalyzeNew } from './app-settings.svelte.ts';
import { getAutoAnalyzeNew } from '../services/tauri.svelte.ts';
import { buildSettingSources, SETTINGS_PAGES } from './settings-rows.ts';
import { ui } from './ui.svelte.ts';
import { buildPaletteGroups } from '../shortcuts/palette-model.ts';

const find = (id: string, audio = false) => buildSettingSources(audio).find((r) => r.id === id)!;

describe('setting rows', () => {
  beforeEach(() => {
    appSettings.autoAnalyzeNew = true;
    appSettings.analysisEngine = 'auto';
    ui.settingsOpen = false;
  });

  it('shows the analyze-new switch with the stored value', () => {
    expect(find('analysis.autoAnalyzeNew').control).toEqual({ kind: 'switch', on: true });
    appSettings.autoAnalyzeNew = false;
    expect(find('analysis.autoAnalyzeNew').control).toEqual({ kind: 'switch', on: false });
  });

  it('flips the switch in the shared store', () => {
    find('analysis.autoAnalyzeNew').change();
    expect(appSettings.autoAnalyzeNew).toBe(false);
  });

  it('picks and cycles the analysis engine', () => {
    find('analysis.engine').change('python');
    expect(appSettings.analysisEngine).toBe('python');
    find('analysis.engine').change();
    expect(appSettings.analysisEngine).toBe('auto');
  });

  it('lists audio rows only when the desktop audio engine is available', () => {
    expect(buildSettingSources(false).some((r) => r.id.startsWith('audio.'))).toBe(false);
    expect(buildSettingSources(true).map((r) => r.id)).toEqual(
      expect.arrayContaining(['audio.sampleRate', 'audio.bufferFrames']),
    );
  });

  it('has an open row per page that opens the sheet on that page', () => {
    for (const page of SETTINGS_PAGES) {
      const row = find(`page.${page.key}`);
      expect(row.label).toBe(`Open settings › ${page.label}`);
      row.change();
      expect(ui.settingsOpen).toBe(true);
      expect(ui.settingsTab).toBe(page.key);
    }
  });

  it('is found from the palette by its setting name', () => {
    const groups = buildPaletteGroups('analyze new', {
      actions: [],
      places: [],
      tracks: [],
      settings: buildSettingSources(false),
    });
    expect(groups.find((g) => g.key === 'settings')!.items[0].name).toBe('Settings: Analyze new tracks');
  });

  it('saves the switch through the service so a reload reads it back', async () => {
    await loadAppSettings();
    expect(appSettings.loaded).toBe(true);
    await setAutoAnalyzeNew(false);
    expect(await getAutoAnalyzeNew()).toBe(false);
    await setAutoAnalyzeNew(true);
    expect(await getAutoAnalyzeNew()).toBe(true);
  });
});
