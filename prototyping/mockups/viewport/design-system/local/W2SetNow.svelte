<script>
  import { SettingsPage, SegmentedControl, Dropdown, Button } from '../../../../viewport/design-system/svelte.js';
  import W2Shortcuts from './W2Shortcuts.svelte';

  let { tab = 'general' } = $props();
  const noop = () => {};

  const ENGINE = [{ value: 'coreaudio', label: 'CoreAudio' }];
  const DEVICES = [
    { value: '', label: 'System default' },
    { value: 'scarlett', label: 'Scarlett 2i2 USB (default)' },
    { value: 'macbook', label: 'MacBook Pro Speakers' },
  ];

  let sections = $derived([
    {
      label: 'General',
      key: 'general',
      items: [
        { title: 'Library database', description: 'SQLite library file used for tracks, playlists, and analysis cache.', control: libraryPath },
        { title: 'Import from Rekordbox', description: 'Bring tracks, playlists, beat grids and cues in from Rekordbox, a Pioneer USB stick or rekordbox.xml.', control: importButton },
        { title: 'App version', description: 'Running build of fourfour / pioneer-test-ui.', control: version },
      ],
    },
    {
      label: 'Audio',
      key: 'audio',
      items: [
        { title: 'Playback engine', description: 'Native macOS audio output for preview playback in the player strip.', control: engine },
        { title: 'Output device', description: 'CoreAudio HAL device for preview playback. Headphone cue / second output is a follow-up.', control: device },
        { title: 'Sample rate', description: 'Device output sample rate for preview playback (default 48 kHz).', control: sampleRate },
        { title: 'Buffer size', description: 'CoreAudio I/O buffer in frames — lower is snappier, higher is safer (default 128).', control: buffer },
      ],
    },
    { label: 'Keyboard shortcuts', key: 'shortcuts', body: shortcuts },
    {
      label: 'Debug',
      key: 'debug',
      items: [
        { title: 'Library contents', description: 'Counts loaded from the SQLite database on startup and after imports.', control: stats },
        { title: 'IPC diagnostic', description: 'Last Tauri invoke metadata for debugging connection issues.', control: ipc },
        { title: 'Reset library database', description: 'Deletes library.db, WAL files, and the ANLZ cache at the path above. Tracks and playlists are wiped; the library folder path in config is kept.', control: reset },
      ],
    },
  ]);
</script>

{#snippet libraryPath()}
  <div class="row"><span class="ff-settings-page__path-value">/Users/merlin/Library/Application Support/fourfour/library.db</span><Button label="Change" size="small" /></div>
{/snippet}
{#snippet importButton()}<Button label="Import…" size="small" />{/snippet}
{#snippet version()}<span class="ff-settings-page__plain-value">0.9.3</span>{/snippet}
{#snippet engine()}<Dropdown options={ENGINE} value="coreaudio" />{/snippet}
{#snippet device()}<Dropdown options={DEVICES} value="scarlett" />{/snippet}
{#snippet sampleRate()}<SegmentedControl items={['44.1 kHz', '48 kHz', '96 kHz']} activeIndex={1} onChange={noop} />{/snippet}
{#snippet buffer()}<SegmentedControl items={['64', '128', '256', '512', '1024']} activeIndex={1} onChange={noop} />{/snippet}
{#snippet shortcuts()}<W2Shortcuts layout="now" />{/snippet}
{#snippet stats()}<span class="ff-settings-page__plain-value">30,412 tracks · 27 playlists</span>{/snippet}
{#snippet ipc()}<span class="ff-settings-page__mono-value">get_state ok 14 ms</span>{/snippet}
{#snippet reset()}<Button label="Reset database…" variant="destructive" />{/snippet}

<div class="shell">
  {#key tab}
    <SettingsPage {sections} fullscreen initialKey={tab} onClose={noop} />
  {/key}
</div>

<style>
  .shell { position: relative; width: 1440px; height: 900px; overflow: hidden; background: var(--ff-bg); border: 1px solid var(--ff-border-hi); border-radius: 10px; font-family: var(--ff-font); color: var(--ff-text); }
  .row { display: flex; align-items: center; gap: var(--ff-space-3); }
  .shell :global(.ff-settings-page__pane) { overflow: hidden; }
</style>
