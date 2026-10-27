<script>
  import { SettingRow, SegmentedControl, Dropdown } from '$ds';
  import { library } from '../../stores/library.svelte.ts';
  import {
    playbackSettings,
    SAMPLE_RATE_OPTIONS,
    BUFFER_FRAME_OPTIONS,
  } from '../../stores/playback-settings.svelte.ts';
  import { changeBufferFrames, changeOutputDevice, changeSampleRate } from '../../stores/app-settings.svelte.ts';
  import { getIsTauri } from '../../services/tauri.svelte.ts';
  import { listPlaybackOutputDevices } from '../../services/playback.svelte.ts';

  let outputDevices = $state([]);
  let advancedOpen = $state(false);
  let busy = $derived(library.analyzing || library.syncing);
  let desktop = getIsTauri();

  $effect(() => {
    if (!desktop) return;
    void listPlaybackOutputDevices().then((devices) => {
      outputDevices = devices;
    });
  });

  let deviceOptions = $derived([
    { value: '', label: 'System default' },
    ...outputDevices.map((dev) => ({
      value: dev.name,
      label: `${dev.name}${dev.is_default ? ' (default)' : ''}`,
    })),
  ]);

  let sampleRateIndex = $derived(
    Math.max(0, SAMPLE_RATE_OPTIONS.findIndex((o) => o.value === playbackSettings.sampleRate)),
  );
  let bufferIndex = $derived(
    Math.max(0, BUFFER_FRAME_OPTIONS.findIndex((o) => o.value === playbackSettings.bufferFrames)),
  );
  let latencyMs = $derived(((playbackSettings.bufferFrames / playbackSettings.sampleRate) * 1000).toFixed(1));
</script>

{#if desktop}
  <div class="ff-settings-card ff-settings-card--status">
    <div class="now">
      <b>{playbackSettings.outputDevice ?? 'System default'}</b>
      <span>CoreAudio · {playbackSettings.sampleRate / 1000} kHz · {playbackSettings.bufferFrames} frames · {latencyMs} ms</span>
    </div>
  </div>

  <div class="ff-settings-section">Output</div>
  <div class="ff-settings-card">
    <SettingRow title="Output device" description="Changing it stops playback.">
      {#snippet control()}
        <Dropdown
          options={deviceOptions}
          value={playbackSettings.outputDevice ?? ''}
          disabled={busy}
          onChange={(name) => void changeOutputDevice(name || null)}
        />
      {/snippet}
    </SettingRow>
  </div>

  <button type="button" class="fold" aria-expanded={advancedOpen} onclick={() => (advancedOpen = !advancedOpen)}>
    <span class="fold__chevron" class:fold__chevron--open={advancedOpen}>›</span> Advanced
  </button>
  {#if advancedOpen}
    <div class="ff-settings-card">
      <SettingRow title="Sample rate" description="Device output rate for preview playback. Default 48 kHz.">
        {#snippet control()}
          <SegmentedControl
            items={SAMPLE_RATE_OPTIONS.map((o) => o.label)}
            activeIndex={sampleRateIndex}
            disabled={busy}
            onChange={(index) => void changeSampleRate(SAMPLE_RATE_OPTIONS[index].value)}
          />
        {/snippet}
      </SettingRow>
      <SettingRow title="Buffer size" description="Frames per I/O buffer. Lower is snappier, higher is safer. Default 128.">
        {#snippet control()}
          <SegmentedControl
            items={BUFFER_FRAME_OPTIONS.map((o) => o.label)}
            activeIndex={bufferIndex}
            disabled={busy}
            onChange={(index) => void changeBufferFrames(BUFFER_FRAME_OPTIONS[index].value)}
          />
        {/snippet}
      </SettingRow>
    </div>
  {/if}
{:else}
  <div class="ff-settings-card ff-settings-card--status">
    <div class="now">
      <b>Playback uses a simulated clock here</b>
      <span>Output device, sample rate and buffer size are available in the desktop app.</span>
    </div>
  </div>
{/if}

<style>
  .now {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-family: var(--ff-font);
    font-size: 13px;
  }

  .now span {
    font-size: 12px;
    color: var(--ff-muted);
  }

  .fold {
    align-self: flex-start;
    margin-top: 10px;
    padding: 0;
    border: none;
    background: none;
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-label);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--ff-muted);
    cursor: pointer;
  }

  .fold:hover {
    color: var(--ff-text);
  }

  .fold__chevron {
    display: inline-block;
    transition: transform 120ms var(--ff-easing);
  }

  .fold__chevron--open {
    transform: rotate(90deg);
  }
</style>
