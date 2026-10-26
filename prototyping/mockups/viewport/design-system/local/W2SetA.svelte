<script>
  import { Dropdown, SegmentedControl, Button, FilterInput, ProgressBar, Spinner, Kbd } from '../../../../viewport/design-system/svelte.js';
  import { createToggle } from '../../../../viewport/design-system/index.js';
  import Vanilla from './Vanilla.svelte';
  import Ico from './Ico.svelte';
  import SRow from './SRow.svelte';
  import W2Shortcuts from './W2Shortcuts.svelte';

  let { page = 'analysis' } = $props();

  const NAV = [
    { key: 'library', label: 'Library', icon: 'database', status: '30,412 tracks' },
    { key: 'analysis', label: 'Analysis', icon: 'cpu', status: 'Running · 120 / 340' },
    { key: 'audio', label: 'Audio', icon: 'volume', status: 'Scarlett 2i2 · 48 kHz' },
    { key: 'keys', label: 'Keyboard', icon: 'keyboard', status: '1 changed' },
    { key: 'advanced', label: 'Advanced', icon: 'sliders', status: '' },
  ];
  const WORKERS = [{ value: 'auto', label: 'Auto (7 of 8 cores)' }];
  const title = $derived(NAV.find((n) => n.key === page)?.label);
</script>

<div class="scrim">
  <div class="sheet">
    <div class="sheet__top">
      <span class="sheet__t">Preferences</span>
      <span class="sheet__k"><Kbd key="⌘," /></span>
      <button class="sheet__x" aria-label="Close"><Ico name="x" size={14} /></button>
    </div>
    <div class="sheet__body">
      <nav class="nav">
        {#each NAV as n}
          <div class="nav__i" class:nav__i--on={n.key === page}>
            <Ico name={n.icon} size={16} />
            <div class="nav__t"><span>{n.label}</span>{#if n.status}<em>{n.status}</em>{/if}</div>
          </div>
        {/each}
      </nav>

      <div class="pg">
        <h2>{title}</h2>
        {#if page === 'analysis'}
          <div class="card card--live">
            <div class="live__row"><Spinner size="small" /><b>Analyzing 120 of 340</b><span>about 4 min left · 3 tracks at a time</span><div class="live__btns"><Button label="Pause" size="small" /><Button label="Cancel" size="small" variant="ghost" /></div></div>
            <ProgressBar value={35} />
            <div class="live__last">Last run: 338 tracks, 0 errors, 2:14 min, built-in engine</div>
          </div>
          <div class="sec">WHEN TRACKS ARRIVE</div>
          <div class="card">
            <SRow compact title="Analyze new tracks on import" desc="Beat grid, key and waveform start when files are added. Off: nothing runs until you choose Analyze.">
              {#snippet control()}<Vanilla make={() => createToggle({ on: true })} />{/snippet}
            </SRow>
          </div>
          <div class="sec">ENGINE</div>
          <div class="card">
            <SRow compact title="Analysis engine" desc="Built-in needs no setup. Python uses DeepRhythm and Essentia and needs the analysis environment.">
              {#snippet control()}<SegmentedControl items={['Built-in', 'Python']} activeIndex={0} />{/snippet}
            </SRow>
            <SRow compact dot title="Parallel workers" desc="Tracks analyzed at the same time.">
              {#snippet control()}<Dropdown options={WORKERS} value="auto" />{/snippet}
            </SRow>
          </div>
          <div class="sec">EXISTING TRACKS</div>
          <div class="card">
            <SRow compact title="Re-analyze library" desc="Values you set by hand (BPM, key, grid, cues) are kept.">
              {#snippet control()}<Button label="Re-analyze…" size="small" />{/snippet}
            </SRow>
          </div>
        {:else}
          <div class="kbar">
            <div class="kbar__f"><FilterInput placeholder="Filter 42 shortcuts…" /></div>
            <SegmentedControl items={['All', 'General', 'Cues', 'Library', 'View']} activeIndex={0} />
            <Button label="Reset all" size="small" />
          </div>
          <p class="knote">Click a shortcut, press the new keys. Esc cancels, Backspace clears.</p>
          <div class="kcols">
            <W2Shortcuts layout="compact" groups={['General', 'Playback', 'Library', 'View']} recordingId="search" conflictId="newpl" />
          </div>
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  .scrim { position: absolute; inset: 0; background: rgba(0, 0, 0, 0.6); display: flex; align-items: center; justify-content: center; }
  .sheet { width: 940px; height: 640px; display: flex; flex-direction: column; background: var(--ff-surface); border: 1px solid var(--ff-border-hi); border-radius: var(--ff-radius-xl); box-shadow: var(--ff-shadow-palette); overflow: hidden; font-family: var(--ff-font); color: var(--ff-text); }
  .sheet__top { flex: none; height: 40px; display: flex; align-items: center; gap: 10px; padding: 0 12px 0 18px; border-bottom: 1px solid var(--ff-border); }
  .sheet__t { flex: 1; font-size: 13px; font-weight: 500; }
  .sheet__x { border: none; background: none; color: var(--ff-text-mid); display: flex; padding: 5px; }
  .sheet__body { flex: 1 1 0; min-height: 0; display: flex; }
  .nav { flex: none; width: 214px; padding: 10px 8px; border-right: 1px solid var(--ff-border); display: flex; flex-direction: column; gap: 2px; background: var(--ff-bg); }
  .nav__i { display: flex; align-items: center; gap: 10px; padding: 8px 10px; border-radius: var(--ff-radius-lg); color: var(--ff-text-mid); }
  .nav__i--on { background: var(--ff-select); color: var(--ff-text); }
  .nav__t { display: flex; flex-direction: column; min-width: 0; }
  .nav__t span { font-size: 13px; }
  .nav__t em { font-style: normal; font-size: 11px; color: var(--ff-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .pg { flex: 1 1 0; min-width: 0; padding: 20px 28px; overflow: hidden; display: flex; flex-direction: column; gap: 8px; }
  h2 { margin: 0 0 6px; font-size: 18px; font-weight: 500; }
  .sec { margin-top: 10px; font-family: var(--ff-font-mono); font-size: var(--ff-type-label); letter-spacing: 0.06em; color: var(--ff-muted); }
  .card { border: 1px solid var(--ff-border); border-radius: var(--ff-radius-xl); background: var(--ff-bg); overflow: hidden; }
  .card--live { padding: 12px 16px; display: flex; flex-direction: column; gap: 10px; background: var(--ff-elev); border-color: var(--ff-border-hi); }
  .live__row { display: flex; align-items: center; gap: 10px; font-size: 13px; }
  .live__row span { color: var(--ff-muted); font-size: 12px; }
  .live__btns { margin-left: auto; display: flex; gap: 6px; }
  .live__last { font-size: 11.5px; color: var(--ff-muted); }
  .kbar { display: flex; align-items: center; gap: 10px; }
  .kbar__f { flex: 1 1 0; }
  .knote { margin: 2px 0 6px; font-size: 11.5px; color: var(--ff-muted); }
  .kcols { column-count: 2; column-gap: 32px; }
</style>
