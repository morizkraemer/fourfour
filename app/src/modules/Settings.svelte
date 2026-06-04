<!--
  Settings.svelte — Fullscreen preferences (prototype SettingsPage layout).
  Opened from the footer status bar settings button or ⌘,.
-->
<script>
  import { SettingsPage, SegmentedControl, Dropdown, Button, Dialog } from '$ds';
  import { ui, closeSettings } from '../stores/ui.svelte.ts';
  import {
    library,
    changeLocalLibraryPath,
    resetLibraryDatabase,
  } from '../stores/library.svelte.ts';
  import {
    playbackSettings,
    PLAYBACK_ENGINE_OPTIONS,
    SAMPLE_RATE_OPTIONS,
    BUFFER_FRAME_OPTIONS,
    setPlaybackSampleRate,
    setPlaybackBufferFrames,
    setPlaybackOutputDevice,
  } from '../stores/playback-settings.svelte.ts';
  import { pickDirectory, getIsTauri } from '../services/tauri.svelte.ts';
  import {
    playbackStop,
    syncPlaybackAudioConfig,
    listPlaybackOutputDevices,
  } from '../services/playback.svelte.ts';

  let outputDevices = $state([]);

  $effect(() => {
    if (!ui.settingsOpen || !getIsTauri()) return;
    void listPlaybackOutputDevices().then((devices) => {
      outputDevices = devices;
    });
  });
  async function handleChangeLibrary() {
    const dir = await pickDirectory();
    if (dir) {
      await changeLocalLibraryPath(dir);
    }
  }

  let busy = $derived(library.analyzing || library.syncing);
  let resetConfirmOpen = $state(false);
  let resetting = $state(false);

  let libraryStats = $derived(
    `${library.trackCount} tracks · ${library.playlists.length} playlists`,
  );

  let sampleRateLabels = $derived(SAMPLE_RATE_OPTIONS.map((o) => o.label));
  let sampleRateActiveIndex = $derived(
    Math.max(0, SAMPLE_RATE_OPTIONS.findIndex((o) => o.value === playbackSettings.sampleRate)),
  );

  let bufferLabels = $derived(BUFFER_FRAME_OPTIONS.map((o) => o.label));
  let bufferActiveIndex = $derived(
    Math.max(0, BUFFER_FRAME_OPTIONS.findIndex((o) => o.value === playbackSettings.bufferFrames)),
  );

  let coreAudioActive = $derived(getIsTauri());

  const engineDropdownOptions = PLAYBACK_ENGINE_OPTIONS.map((o) => ({
    value: o.id,
    label: o.label,
  }));

  let outputDeviceOptions = $derived([
    { value: '', label: 'System default' },
    ...outputDevices.map((dev) => ({
      value: dev.name,
      label: `${dev.name}${dev.is_default ? ' (default)' : ''}`,
    })),
  ]);

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
          description: 'Native macOS audio output for preview playback in the player strip.',
          control: engineControl,
        },
        ...(coreAudioActive
          ? [
              {
                title: 'Output device',
                description:
                  'CoreAudio HAL device for preview playback. Headphone cue / second output is a follow-up.',
                control: outputDeviceControl,
              },
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
      label: 'Debug',
      key: 'debug',
      items: [
        {
          title: 'Library contents',
          description: 'Counts loaded from the SQLite database on startup and after imports.',
          control: libraryStatsControl,
        },
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
        {
          title: 'Reset library database',
          description:
            'Deletes library.db, WAL files, and the ANLZ cache at the path above. Tracks and playlists are wiped; the library folder path in config is kept.',
          control: resetLibraryControl,
        },
      ],
    },
  ]);

  async function commitResetLibrary() {
    resetting = true;
    try {
      await resetLibraryDatabase();
      resetConfirmOpen = false;
    } catch {
      /* statusMessage set in store */
    } finally {
      resetting = false;
    }
  }

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

{#snippet libraryStatsControl()}
  <span class="ff-settings-page__plain-value">{libraryStats}</span>
{/snippet}

{#snippet resetLibraryControl()}
  <Button
    label="Reset database…"
    variant="destructive"
    disabled={busy || resetting}
    onclick={() => (resetConfirmOpen = true)}
  />
{/snippet}

{#snippet ipcControl()}
  <span class="ff-settings-page__mono-value">{library.ipcDiagnostic || '—'}</span>
{/snippet}

{#snippet errorControl()}
  <span class="ff-settings-page__mono-value">{library.tauriError}</span>
{/snippet}

{#snippet engineControl()}
  <Dropdown options={engineDropdownOptions} value="coreaudio" />
{/snippet}

{#snippet outputDeviceControl()}
  <Dropdown
    options={outputDeviceOptions}
    value={playbackSettings.outputDevice ?? ''}
    disabled={busy}
    onChange={(name) => {
      const device = name || null;
      if (device === playbackSettings.outputDevice) return;
      void (async () => {
        await playbackStop();
        setPlaybackOutputDevice(device);
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

<Dialog
  open={resetConfirmOpen}
  title="Reset library database?"
  bodyText="This permanently deletes all tracks, playlists, and analysis cache for the current library file. Audio files on disk are not removed. This cannot be undone."
  onClose={() => {
    if (!resetting) resetConfirmOpen = false;
  }}
>
  {#snippet actions()}
    <Button
      label="Cancel"
      variant="ghost"
      disabled={resetting}
      onclick={() => (resetConfirmOpen = false)}
    />
    <Button
      label={resetting ? 'Resetting…' : 'Reset'}
      variant="destructive"
      disabled={resetting}
      onclick={() => void commitResetLibrary()}
    />
  {/snippet}
</Dialog>

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
