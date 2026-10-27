<!--
  Settings.svelte — Preferences sheet over the library: a page list by object
  (Library, Analysis, Audio, Keyboard, Advanced) and one short page at a time.
  Opened from the footer status bar gear, ⌘, or the command palette.
-->
<script>
  import { SettingsSheet } from '$ds';
  import { ui, closeSettings } from '../stores/ui.svelte.ts';
  import { library } from '../stores/library.svelte.ts';
  import { appSettings, loadAppSettings } from '../stores/app-settings.svelte.ts';
  import { playbackSettings } from '../stores/playback-settings.svelte.ts';
  import { SETTINGS_PAGES } from '../stores/settings-rows.ts';
  import { shortcuts } from '../shortcuts/index.ts';
  import { formatBinding } from '../shortcuts/keybinding.ts';
  import { getIsTauri } from '../services/tauri.svelte.ts';
  import RekordboxImport from './RekordboxImport.svelte';
  import LibraryPage from './settings/LibraryPage.svelte';
  import AnalysisPage from './settings/AnalysisPage.svelte';
  import AudioPage from './settings/AudioPage.svelte';
  import ShortcutsSettings from './ShortcutsSettings.svelte';
  import AdvancedPage from './settings/AdvancedPage.svelte';

  $effect(() => {
    if (ui.settingsOpen) void loadAppSettings();
  });

  let activeKey = $derived(
    SETTINGS_PAGES.some((p) => p.key === ui.settingsTab) ? ui.settingsTab : SETTINGS_PAGES[0].key,
  );

  function status(key) {
    switch (key) {
      case 'library':
        return `${library.trackCount} tracks · ${library.playlists.length} playlists`;
      case 'analysis':
        if (library.analyzing) {
          return `Running · ${library.analysisProgress.current} / ${library.analysisProgress.total}`;
        }
        return appSettings.autoAnalyzeNew ? 'New tracks analyzed' : 'New tracks wait for you';
      case 'audio':
        if (!getIsTauri()) return 'Desktop app only';
        return `${playbackSettings.outputDevice ?? 'System default'} · ${playbackSettings.sampleRate / 1000} kHz`;
      case 'keyboard':
        return shortcuts.hasCustom() ? 'Customized' : `${shortcuts.actions.length} shortcuts`;
      default:
        return '';
    }
  }

  let pages = $derived(SETTINGS_PAGES.map((p) => ({ key: p.key, label: p.label, status: status(p.key) })));
  let hint = $derived(formatBinding(shortcuts.bindingOf('settings.toggle')));
</script>

<SettingsSheet
  open={ui.settingsOpen}
  {pages}
  {activeKey}
  shortcutHint={hint === 'Unassigned' ? '' : hint}
  onSelect={(key) => (ui.settingsTab = key)}
  onClose={closeSettings}
>
  {#if activeKey === 'library'}
    <LibraryPage />
  {:else if activeKey === 'analysis'}
    <AnalysisPage />
  {:else if activeKey === 'audio'}
    <AudioPage />
  {:else if activeKey === 'keyboard'}
    <ShortcutsSettings />
  {:else}
    <AdvancedPage />
  {/if}
</SettingsSheet>

<RekordboxImport />
