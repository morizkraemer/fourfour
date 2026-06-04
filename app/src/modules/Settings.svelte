<!--
  Settings.svelte — Fullscreen preferences (prototype SettingsPage layout).
  Opened from the footer status bar settings button or ⌘,.
-->
<script>
  import { SettingsPage, SegmentedControl, Button } from '$ds';
  import { ui, closeSettings } from '../stores/ui.svelte.ts';
  import {
    library,
    changeLocalLibraryPath,
  } from '../stores/library.svelte.ts';
  import {
    playbackSettings,
    PLAYBACK_ENGINE_OPTIONS,
    SAMPLE_RATE_OPTIONS,
    BUFFER_FRAME_OPTIONS,
    availablePlaybackEngines,
    setPlaybackEngine,
    setPlaybackSampleRate,
    setPlaybackBufferFrames,
  } from '../stores/playback-settings.svelte.ts';
  import { pickDirectory, getIsTauri } from '../services/tauri.svelte.ts';
  import { playbackStop, syncPlaybackAudioConfig } from '../services/playback.svelte.ts';
  async function handleChangeLibrary() {
    const dir = await pickDirectory();
    if (dir) {
      await changeLocalLibraryPath(dir);
    }
  }

  let busy = $derived(library.analyzing || library.syncing);

  let engineOptions = $derived(
    PLAYBACK_ENGINE_OPTIONS.filter((o) => availablePlaybackEngines().includes(o.id)),
  );
  let engineLabels = $derived(engineOptions.map((o) => o.label));
  let engineActiveIndex = $derived(
    Math.max(0, engineOptions.findIndex((o) => o.id === playbackSettings.engine)),
  );

  let sampleRateLabels = $derived(SAMPLE_RATE_OPTIONS.map((o) => o.label));
  let sampleRateActiveIndex = $derived(
    Math.max(0, SAMPLE_RATE_OPTIONS.findIndex((o) => o.value === playbackSettings.sampleRate)),
  );

  let bufferLabels = $derived(BUFFER_FRAME_OPTIONS.map((o) => o.label));
  let bufferActiveIndex = $derived(
    Math.max(0, BUFFER_FRAME_OPTIONS.findIndex((o) => o.value === playbackSettings.bufferFrames)),
  );

  let coreAudioActive = $derived(
    getIsTauri() && playbackSettings.engine === 'coreaudio',
  );

  async function applyCoreAudioConfig() {
    if (!coreAudioActive) return;
    await syncPlaybackAudioConfig();
  }

  const sections = $derived([
    {
      label: 'General',
      key: 'general',
      items: [
        {
          title: 'Library database',
          description: 'SQLite library file used for tracks, playlists, and analysis cache.',
          control: libraryPathControl,
        },
        {
          title: 'App version',
          description: 'Running build of fourfour / pioneer-test-ui.',
          control: versionControl,
        },
      ],
    },
    {
      label: 'Audio',
      key: 'audio',
      items: [
        {
          title: 'Playback engine',
          description: getIsTauri()
            ? 'Preview transport in the player strip — CoreAudio plays files; mock fakes position without sound.'
            : 'Browser preview uses mock transport only (CoreAudio requires the desktop app).',
          control: engineControl,
        },
        ...(coreAudioActive
          ? [
              {
                title: 'Sample rate',
                description: 'Device output sample rate for preview playback (default 48 kHz).',
                control: sampleRateControl,
              },
              {
                title: 'Buffer size',
                description: 'CoreAudio I/O buffer in frames — lower is snappier, higher is safer (default 128).',
                control: bufferControl,
              },
            ]
          : []),
      ],
    },
    {
      label: 'Diagnostics',
      key: 'diagnostics',
      items: [
        {
          title: 'IPC diagnostic',
          description: 'Last Tauri invoke metadata for debugging connection issues.',
          control: ipcControl,
        },
        ...(library.tauriError
          ? [
              {
                title: 'Last error',
                description: 'Most recent backend error surfaced to the UI.',
                control: errorControl,
              },
            ]
          : []),
      ],
    },
  ]);

  function onSettingsKeydown(e) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      closeSettings();
    }
  }
</script>

{#snippet libraryPathControl()}
  <div class="ff-settings-path-row">
    <span class="ff-settings-page__path-value" title={library.libraryPath}>
      {library.libraryPath || 'Not set'}
    </span>
    <Button
      label="Change"
      size="small"
      disabled={busy}
      onclick={handleChangeLibrary}
    />
  </div>
{/snippet}

{#snippet versionControl()}
  <span class="ff-settings-page__plain-value">{library.appVersion || '—'}</span>
{/snippet}

{#snippet ipcControl()}
  <span class="ff-settings-page__mono-value">{library.ipcDiagnostic || '—'}</span>
{/snippet}

{#snippet errorControl()}
  <span class="ff-settings-page__mono-value">{library.tauriError}</span>
{/snippet}

{#snippet engineControl()}
  <SegmentedControl
    items={engineLabels}
    activeIndex={engineActiveIndex}
    disabled={busy}
    onChange={(index) => {
      const opt = engineOptions[index];
      if (!opt || opt.id === playbackSettings.engine) return;
      void (async () => {
        await playbackStop();
        setPlaybackEngine(opt.id);
        await applyCoreAudioConfig();
      })();
    }}
  />
{/snippet}

{#snippet sampleRateControl()}
  <SegmentedControl
    items={sampleRateLabels}
    activeIndex={sampleRateActiveIndex}
    disabled={busy}
    onChange={(index) => {
      const opt = SAMPLE_RATE_OPTIONS[index];
      if (!opt || opt.value === playbackSettings.sampleRate) return;
      void (async () => {
        await playbackStop();
        setPlaybackSampleRate(opt.value);
        await applyCoreAudioConfig();
      })();
    }}
  />
{/snippet}

{#snippet bufferControl()}
  <SegmentedControl
    items={bufferLabels}
    activeIndex={bufferActiveIndex}
    disabled={busy}
    onChange={(index) => {
      const opt = BUFFER_FRAME_OPTIONS[index];
      if (!opt || opt.value === playbackSettings.bufferFrames) return;
      void (async () => {
        await playbackStop();
        setPlaybackBufferFrames(opt.value);
        await applyCoreAudioConfig();
      })();
    }}
  />
{/snippet}

{#if ui.settingsOpen}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="ff-settings-screen"
    role="dialog"
    aria-modal="true"
    aria-label="Preferences"
    tabindex="-1"
    onkeydown={onSettingsKeydown}
  >
    <SettingsPage {sections} fullscreen onClose={closeSettings} />
  </div>
{/if}

<style>
  .ff-settings-screen {
    position: fixed;
    inset: 0;
    z-index: 10000;
    display: flex;
    flex-direction: column;
    background: var(--ff-bg);
    overflow: hidden;
  }

  .ff-settings-path-row {
    display: flex;
    align-items: center;
    gap: var(--ff-space-3);
  }
</style>
