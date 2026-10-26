<script>
  import {
    SettingsPage, Dropdown, SegmentedControl, Button, FilterInput, Kbd, Icon, Spinner, StatusDot,
  } from '../../../../viewport/design-system/svelte.js';
  import Win from './Win.svelte';
  import Frame from './Frame.svelte';
  import Ico from './Ico.svelte';
  import SRow from './SRow.svelte';
  import Tog from './Tog.svelte';
  import ShortcutsTable from './ShortcutsTable.svelte';

  let { variant = 'now' } = $props();

  const META = {
    now: {
      tag: 'NOW',
      title: 'Settings today',
      model: 'Full-window takeover. Left nav, one 680px column, a card per section; each row is a long label plus a small control.',
      good: ['Familiar two-pane preferences layout', 'Card per section reads clearly'],
      cost: [
        'Takes over the whole window (no library behind it) and leaves the right half of the screen empty; the column hugs the left',
        'Rows are tall (label plus a two-line description for every setting): 4 settings fill a screen',
        'Back button, "Preferences" title, section title and card title all say the same thing',
        'Audio, library, debug and reset sit side by side with equal weight; no analysis section; shortcuts is a separate long list with no search',
      ],
    },
    a: {
      tag: 'A',
      title: 'Preferences sheet with a toolbar',
      model: 'A centred sheet over the library (⌘, opens, Esc closes). Icon toolbar replaces the side nav. One compact row per setting. Two tabs shown: Analysis and Shortcuts.',
      good: [
        'You keep the library in sight while changing something; fast open and close',
        'Toolbar tabs are the whole map: six pages, none long; fixed size, no empty half-screen',
        'Shortcuts tab: filter, click-to-record, inline conflict with Replace / Cancel (shown)',
      ],
      cost: [
        'Fixed-size sheet: a long page (shortcuts, 40+ rows) needs its own scroll',
        'No search across settings; one more window-like thing in a one-window app',
      ],
    },
    b: {
      tag: 'B',
      title: 'One searchable settings page',
      model: 'Everything on one scrolling page, in order, with a search field and an index on the left. Values that differ from default get a dot and a reset.',
      good: [
        'Search is the map: "buffer", "key", "usb" jump to a row; no hunting through tabs',
        'Compact rows, one line of description; reads like a config, which suits a pro tool',
        'Shortcuts become one more section; "changed from default" dots show what you touched',
      ],
      cost: [
        'Still a window takeover (could open as a sheet; the page works either way)',
        'Long page: needs a sticky index and a good scroll-spy; lots of rows compete for attention',
      ],
    },
    c: {
      tag: 'C',
      title: 'Settings where they matter',
      model: 'Frequent settings live next to the thing they change: audio at the player, analysis at its status chip, shortcuts as a ⌘/ overlay. Preferences shrinks to Library and Advanced.',
      good: [
        'No hunting: the audio output is one click from the player, the analysis engine one click from the progress chip',
        'Shortcuts overlay doubles as a cheat sheet (also reachable from ⌘K)',
        'Most settings are tried while looking at the effect (buffer size while playing)',
      ],
      cost: [
        'Settings are scattered: no single place to see everything, no search across them (⌘K can find them)',
        'More popovers to build and keep consistent; the thin Preferences still needs a page',
      ],
    },
  };
  const m = $derived(META[variant]);

  const DEVICES = [{ value: 'scarlett', label: 'Scarlett 2i2 USB' }];
  const RATES = [{ value: '48', label: '48 kHz' }];
  const BUF = [{ value: '256', label: '256 frames (5.3 ms)' }];
  const NOWBUF = [{ value: '128', label: '128 frames (2.7 ms)' }];
  const WORKERS = [{ value: 'auto', label: 'Auto (7 of 8 cores)' }];
  const NEWPL = [{ value: 'keep', label: 'Keep updated' }];
</script>

{#snippet nowGeneral()}
  <div class="ff-settings-page__fields-container">
    <div class="ff-settings-page__field">
      <div class="ff-settings-page__field-texts"><span class="ff-settings-page__field-label">Library database</span><span class="ff-settings-page__field-desc">SQLite library file used for tracks, playlists, and analysis cache.</span></div>
      <div class="ff-settings-page__field-control"><span class="ff-settings-page__path-value">/Users/merlin/Music/fourfour/library.db</span></div>
    </div>
    <div class="ff-settings-page__field">
      <div class="ff-settings-page__field-texts"><span class="ff-settings-page__field-label">Import from Rekordbox</span><span class="ff-settings-page__field-desc">Read playlists, cues and beat grids from a Rekordbox export.</span></div>
      <div class="ff-settings-page__field-control"><Button label="Import…" /></div>
    </div>
    <div class="ff-settings-page__field">
      <div class="ff-settings-page__field-texts"><span class="ff-settings-page__field-label">App version</span><span class="ff-settings-page__field-desc">Running build of fourfour / pioneer-test-ui.</span></div>
      <div class="ff-settings-page__field-control"><span class="ff-settings-page__plain-value">0.9.3 (4182)</span></div>
    </div>
  </div>
{/snippet}
{#snippet nowAudio()}
  <div class="ff-settings-page__fields-container">
    <div class="ff-settings-page__field">
      <div class="ff-settings-page__field-texts"><span class="ff-settings-page__field-label">Output device</span><span class="ff-settings-page__field-desc">Audio device for preview playback in the player strip. System default follows macOS.</span></div>
      <div class="ff-settings-page__field-control"><Dropdown options={DEVICES} value="scarlett" /></div>
    </div>
    <div class="ff-settings-page__field">
      <div class="ff-settings-page__field-texts"><span class="ff-settings-page__field-label">Sample rate</span><span class="ff-settings-page__field-desc">Device output sample rate for preview playback (default 48 kHz).</span></div>
      <div class="ff-settings-page__field-control"><Dropdown options={RATES} value="48" /></div>
    </div>
    <div class="ff-settings-page__field">
      <div class="ff-settings-page__field-texts"><span class="ff-settings-page__field-label">Buffer size</span><span class="ff-settings-page__field-desc">CoreAudio I/O buffer in frames. Lower is snappier, higher is safer (default 128).</span></div>
      <div class="ff-settings-page__field-control"><Dropdown options={NOWBUF} value="128" /></div>
    </div>
  </div>
{/snippet}
{#snippet nowKeys()}
  <div class="nowkeys"><ShortcutsTable cols={1} showConflict={false} /></div>
{/snippet}
{#snippet nowDebug()}
  <div class="ff-settings-page__fields-container">
    <div class="ff-settings-page__field">
      <div class="ff-settings-page__field-texts"><span class="ff-settings-page__field-label">Library contents</span><span class="ff-settings-page__field-desc">Counts loaded from the SQLite database on startup and after imports.</span></div>
      <div class="ff-settings-page__field-control"><span class="ff-settings-page__mono-value">30,412 tracks · 27 playlists</span></div>
    </div>
    <div class="ff-settings-page__field">
      <div class="ff-settings-page__field-texts"><span class="ff-settings-page__field-label">Reset library database</span><span class="ff-settings-page__field-desc">Deletes all tracks, playlists and analysis from the library file.</span></div>
      <div class="ff-settings-page__field-control"><Button label="Reset database…" variant="destructive" /></div>
    </div>
  </div>
{/snippet}

{#snippet analysisRows(compact)}
  <SRow title="Analysis engine" desc="Built-in needs no setup. Python uses DeepRhythm and Essentia and needs the analysis environment." {compact} dot>
    {#snippet control()}<SegmentedControl items={['Built-in', 'Python']} activeIndex={0} />{/snippet}
  </SRow>
  <SRow title="Parallel workers" desc="Tracks analyzed at the same time." {compact}>
    {#snippet control()}<Dropdown options={WORKERS} value="auto" />{/snippet}
  </SRow>
  <SRow title="Analyze new tracks on import" desc="Beat grid, key and waveform start as soon as files are added." {compact}>
    {#snippet control()}<Tog on />{/snippet}
  </SRow>
  <SRow title="Re-analyze library" desc="30,412 tracks, about 4 h on this machine." {compact}>
    {#snippet control()}<Button label="Re-analyze…" size="small" />{/snippet}
  </SRow>
  <SRow title="Last run" desc="338 tracks · 0 errors · 2:14 min." {compact}>
    {#snippet control()}<span class="mono">Built-in 0.9</span>{/snippet}
  </SRow>
{/snippet}

{#snippet audioRows(compact)}
  <SRow title="Output device" desc="Preview playback in the player strip." {compact}>
    {#snippet control()}<Dropdown options={DEVICES} value="scarlett" />{/snippet}
  </SRow>
  <SRow title="Sample rate" {compact}>
    {#snippet control()}<Dropdown options={RATES} value="48" />{/snippet}
  </SRow>
  <SRow title="Buffer size" desc="Lower is snappier, higher is safer." {compact} dot>
    {#snippet control()}<Dropdown options={BUF} value="256" />{/snippet}
  </SRow>
{/snippet}

{#snippet usbRows(compact)}
  <SRow title="Playlists added to a stick" desc="Keep updated follows the library. Stay as is leaves the stick copy alone." {compact}>
    {#snippet control()}<Dropdown options={NEWPL} value="keep" />{/snippet}
  </SRow>
  <SRow title="Sync when a stick is plugged in" desc="Only playlists placed on the stick. Never deletes files fourfour did not write." {compact}>
    {#snippet control()}<Tog on />{/snippet}
  </SRow>
{/snippet}

<Frame tag={m.tag} title={m.title} model={m.model} good={m.good} cost={m.cost}>
  {#if variant === 'now'}
    <div class="shell">
      <SettingsPage
        fullscreen
        initialKey="audio"
        onClose={() => {}}
        sections={[
          { label: 'General', key: 'general', body: nowGeneral },
          { label: 'Audio', key: 'audio', body: nowAudio },
          { label: 'Keyboard shortcuts', key: 'shortcuts', body: nowKeys },
          { label: 'Debug', key: 'debug', body: nowDebug },
        ]}
      />
    </div>
  {:else if variant === 'a'}
    <div class="stage">
      {#each [0, 1] as which}
        <div class="sheet">
          <div class="sheet__title"><span class="sheet__dots"><i></i><i></i><i></i></span><span>Preferences</span></div>
          <div class="sheet__tabs">
            {#each [['General', 'sliders2'], ['Library', 'database'], ['Playback', 'volume'], ['Analysis', 'cpu'], ['USB', 'usb'], ['Shortcuts', 'keyboard']] as [label, icon]}
              <span class="sheet__tab" class:sheet__tab--on={(which === 0 && label === 'Analysis') || (which === 1 && label === 'Shortcuts')}><Ico name={icon} size={18} />{label}</span>
            {/each}
          </div>
          <div class="sheet__body">
            {#if which === 0}
              <div class="sheet__sec">ENGINE</div>
              <div class="sheet__card">{@render analysisRows(true)}</div>
              <div class="sheet__note">Changes apply to the next track analyzed. Already analyzed tracks keep their results.</div>
            {:else}
              <div class="sheet__bar">
                <FilterInput placeholder="Filter shortcuts…" />
                <Button label="Reset all" size="small" />
              </div>
              <div class="sheet__scroll"><ShortcutsTable cols={1} groups={['Playback', 'Library']} compact /></div>
              <div class="sheet__fade"></div>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {:else if variant === 'b'}
    <div class="page">
      <div class="page__top">
        <button class="page__back"><Ico name="chevron-left" size={14} /> Back</button>
        <span class="page__t">Settings</span>
        <div class="page__search"><FilterInput placeholder="Search settings: buffer, key, usb…" /></div>
        <Kbd key="Esc" />
      </div>
      <div class="page__main">
        <nav class="page__idx">
          {#each ['General', 'Library', 'Playback', 'Analysis', 'USB', 'Shortcuts', 'Advanced'] as s}
            <span class="page__ix" class:page__ix--on={s === 'Analysis'}>{s}</span>
          {/each}
        </nav>
        <div class="page__scroll">
          <div class="page__col">
            <div class="page__h">PLAYBACK</div>
            <div class="page__card">{@render audioRows(true)}</div>
            <div class="page__h">ANALYSIS</div>
            <div class="page__card">{@render analysisRows(true)}</div>
            <div class="page__h">USB</div>
            <div class="page__card">{@render usbRows(true)}</div>
            <div class="page__h">KEYBOARD SHORTCUTS</div>
            <div class="page__bar"><FilterInput placeholder="Filter shortcuts…" /><Button label="Reset all" size="small" /></div>
            <ShortcutsTable cols={1} groups={['Playback', 'Library']} compact />
          </div>
          <div class="page__fade"></div>
        </div>
      </div>
    </div>
  {:else}
    <Win cols="mid">
      {#snippet statusExtra()}
        <span class="chipx chipx--on"><Spinner size="small" /> Analyzing 120 / 340 <Ico name="chevron-down" size={10} /></span>
      {/snippet}
      {#snippet overlay()}
        <div class="scrim"></div>
        <div class="pop pop--keys">
          <div class="pop__head"><span class="pop__t">Keyboard shortcuts</span><span class="pop__s">click a key to change it</span><span class="pop__sp"></span><FilterInput placeholder="Filter…" /><Button label="Reset all" size="small" /></div>
          <div class="pop__body"><ShortcutsTable cols={2} compact /></div>
          <div class="pop__foot"><Kbd key="⌘/" /> opens this anywhere · also in ⌘K</div>
        </div>
        <div class="pop pop--audio">
          <div class="pop__head"><Ico name="volume" size={14} /><span class="pop__t">Playback output</span></div>
          <div class="pop__form">
            <label>Device</label><Dropdown options={DEVICES} value="scarlett" />
            <label>Sample rate</label><SegmentedControl items={['44.1', '48', '96']} activeIndex={1} />
            <label>Buffer</label><SegmentedControl items={['64', '128', '256', '512']} activeIndex={1} />
          </div>
          <div class="pop__foot">Latency 2.7 ms · applies immediately</div>
          <span class="pop__tail pop__tail--down" style="left:200px"></span>
        </div>
        <div class="pop pop--an">
          <div class="pop__head"><Ico name="cpu" size={14} /><span class="pop__t">Analysis</span><span class="pop__sp"></span><Button label="Pause" size="compact" /></div>
          <div class="pop__prog"><div><i style="width:35%"></i></div><span>120 / 340 tracks · 2:14 left</span></div>
          <div class="pop__form">
            <label>Engine</label><SegmentedControl items={['Built-in', 'Python']} activeIndex={0} />
            <label>Workers</label><Dropdown options={WORKERS} value="auto" />
            <label>On import</label><span class="pop__tg"><Tog on /> analyze automatically</span>
          </div>
          <div class="pop__foot"><span>Re-analyze library…</span> <span>Last run: 0 errors</span></div>
          <span class="pop__tail pop__tail--down" style="right:90px"></span>
        </div>
      {/snippet}
    </Win>
  {/if}
</Frame>

<style>
  .mono { font-family: var(--ff-font-mono); font-size: 11px; color: var(--ff-text-mid); }
  .shell { position: relative; width: 1440px; height: 900px; background: var(--ff-bg); border: 1px solid var(--ff-border-hi); border-radius: 10px; overflow: hidden; }
  .shell :global(.nowkeys) { max-width: 600px; overflow: hidden; }
  .shell :global(.ff-settings-page) { background: var(--ff-bg); }

  .stage { display: grid; grid-template-columns: 1fr 1fr; gap: 40px; align-items: start; width: 1440px; height: 900px; padding: 40px 50px; box-sizing: border-box; background: radial-gradient(ellipse at 50% 0, #151515, var(--ff-bg) 70%); border: 1px solid var(--ff-border-hi); border-radius: 10px; }
  .sheet { display: flex; flex-direction: column; height: 640px; background: var(--ff-surface); border: 1px solid var(--ff-border-hi); border-radius: 10px; box-shadow: var(--ff-shadow-dialog); overflow: hidden; font-family: var(--ff-font); }
  .sheet__title { height: 38px; display: flex; align-items: center; justify-content: center; position: relative; font-size: 12.5px; color: var(--ff-text-mid); border-bottom: 1px solid var(--ff-border); }
  .sheet__dots { position: absolute; left: 14px; display: flex; gap: 8px; }
  .sheet__dots i { width: 12px; height: 12px; border-radius: 50%; background: var(--ff-elev-hi); }
  .sheet__tabs { display: flex; gap: 4px; justify-content: center; padding: 10px 14px 8px; border-bottom: 1px solid var(--ff-border); background: var(--ff-bg); }
  .sheet__tab { width: 84px; height: 52px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 4px; font-size: 11px; color: var(--ff-muted); border-radius: var(--ff-radius-lg); }
  .sheet__tab--on { background: var(--ff-select); color: var(--ff-text); }
  .sheet__body { position: relative; flex: 1; min-height: 0; padding: 18px 22px; overflow: hidden; display: flex; flex-direction: column; gap: 10px; }
  .sheet__sec { font-family: var(--ff-font-mono); font-size: var(--ff-type-label-sm); letter-spacing: 0.06em; color: var(--ff-faint); }
  .sheet__card { background: var(--ff-bg); border: 1px solid var(--ff-border); border-radius: 8px; }
  .sheet__note { font-size: 11.5px; color: var(--ff-muted); padding: 0 4px; }
  .sheet__bar { display: flex; gap: 10px; align-items: center; }
  .sheet__bar :global(.ff-filter-input) { flex: 1; }
  .sheet__scroll { overflow: hidden; flex: 1; }
  .sheet__fade { position: absolute; left: 0; right: 0; bottom: 0; height: 60px; background: linear-gradient(transparent, var(--ff-surface)); pointer-events: none; }

  .page { display: flex; flex-direction: column; width: 1440px; height: 900px; background: var(--ff-bg); border: 1px solid var(--ff-border-hi); border-radius: 10px; overflow: hidden; font-family: var(--ff-font); }
  .page__top { flex: none; height: 50px; display: flex; align-items: center; gap: 14px; padding: 0 18px 0 80px; border-bottom: 1px solid var(--ff-border); }
  .page__back { display: flex; align-items: center; gap: 2px; border: none; background: none; color: var(--ff-text-mid); font-size: 12.5px; font-family: var(--ff-font); }
  .page__t { font-size: 14px; font-weight: 500; width: 120px; }
  .page__search { flex: 1; max-width: 460px; margin: 0 auto 0 0; }
  .page__main { flex: 1; min-height: 0; display: flex; }
  .page__idx { flex: none; width: 200px; padding: 18px 10px; border-right: 1px solid var(--ff-border); display: flex; flex-direction: column; gap: 2px; }
  .page__ix { padding: 6px 10px; font-size: 12.5px; color: var(--ff-text-mid); border-radius: var(--ff-radius-md); }
  .page__ix--on { background: var(--ff-select); color: var(--ff-text); }
  .page__scroll { position: relative; flex: 1; min-width: 0; overflow: hidden; display: flex; justify-content: center; padding: 22px 0 0; }
  .page__col { width: 760px; display: flex; flex-direction: column; gap: 8px; }
  .page__h { font-family: var(--ff-font-mono); font-size: var(--ff-type-label-sm); letter-spacing: 0.06em; color: var(--ff-faint); margin-top: 10px; }
  .page__card { background: var(--ff-surface); border: 1px solid var(--ff-border); border-radius: 8px; }
  .page__bar { display: flex; gap: 10px; }
  .page__bar :global(.ff-filter-input) { flex: 1; }
  .page__fade { position: absolute; left: 0; right: 0; bottom: 0; height: 80px; background: linear-gradient(transparent, var(--ff-bg)); pointer-events: none; }

  .scrim { position: absolute; inset: 0; background: rgba(0, 0, 0, 0.55); }
  .chipx { display: inline-flex; align-items: center; gap: 6px; height: 18px; padding: 0 8px; border: 1px solid var(--ff-border); border-radius: 3px; font-size: 11px; color: var(--ff-text-mid); font-family: var(--ff-font-mono); }
  .chipx--on { border-color: var(--ff-border-hi); background: var(--ff-elev); color: var(--ff-text); }
  .pop { position: absolute; background: var(--ff-elev); border: 1px solid var(--ff-border-hi); border-radius: var(--ff-radius-xl); box-shadow: var(--ff-shadow-palette); font-family: var(--ff-font); display: flex; flex-direction: column; }
  .pop--keys { left: 262px; top: 70px; width: 580px; }
  .pop--audio { left: 852px; bottom: 120px; width: 268px; }
  .pop--audio .pop__form { grid-template-columns: 70px 1fr; }
  .pop--an { right: 16px; bottom: 34px; width: 296px; }
  .pop__head { display: flex; align-items: center; gap: 8px; padding: 12px 14px; border-bottom: 1px solid var(--ff-border); color: var(--ff-text-mid); }
  .pop__t { font-size: 13px; font-weight: 500; color: var(--ff-text); }
  .pop__s { font-size: 11.5px; color: var(--ff-muted); }
  .pop__sp { flex: 1; }
  .pop__head :global(.ff-filter-input) { width: 150px; }
  .pop__body { flex: 1; overflow: hidden; padding: 12px 16px 0; }
  .pop__foot { display: flex; justify-content: space-between; gap: 8px; padding: 9px 14px; border-top: 1px solid var(--ff-border); font-size: 11.5px; color: var(--ff-muted); }
  .pop__form { display: grid; grid-template-columns: 78px 1fr; gap: 10px 8px; align-items: center; padding: 14px; font-size: 12px; color: var(--ff-text-mid); }
  .pop__form :global(.ff-dropdown) { min-width: 0; width: 100%; }
  .pop__tg { display: flex; align-items: center; gap: 8px; font-size: 11.5px; }
  .pop__prog { padding: 12px 14px 0; display: flex; flex-direction: column; gap: 6px; font-size: 11.5px; color: var(--ff-text-mid); }
  .pop__prog div { height: 4px; background: var(--ff-bg); border-radius: 2px; overflow: hidden; }
  .pop__prog i { display: block; height: 100%; background: var(--ff-accent); }
  .pop__tail { position: absolute; width: 10px; height: 10px; background: var(--ff-elev); border-right: 1px solid var(--ff-border-hi); border-bottom: 1px solid var(--ff-border-hi); transform: rotate(45deg); }
  .pop__tail--down { bottom: -6px; }
</style>
