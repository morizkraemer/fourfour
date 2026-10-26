<script>
  import { Dropdown, SegmentedControl, Button, LevelMeter, FilterInput } from '../../../../viewport/design-system/svelte.js';
  import { createToggle } from '../../../../viewport/design-system/index.js';
  import Vanilla from './Vanilla.svelte';
  import Ico from './Ico.svelte';

  let { width = 400 } = $props();

  const DEVICES = [{ value: 'scarlett', label: 'Scarlett 2i2 USB' }];
  const WORKERS = [{ value: 'auto', label: 'Auto (7 of 8 cores)' }];
</script>

<div class="p" style="width:{width}px">
  <div class="p__head">
    <span class="p__t">Settings</span>
    <div class="p__f"><FilterInput placeholder="Find a setting…" /></div>
    <button class="p__x" aria-label="Close"><Ico name="x" size={14} /></button>
  </div>

  <div class="p__scroll">
    <section class="s">
      <header><Ico name="chevron-down" size={13} /><b>Audio</b><span>Scarlett 2i2 · 48 kHz · 128</span></header>
      <div class="s__b">
        <div class="r"><label>Output</label><div class="r__c"><Dropdown options={DEVICES} value="scarlett" /></div></div>
        <div class="r"><label>Sample rate</label><div class="r__c"><SegmentedControl items={['44.1', '48', '96']} activeIndex={1} /></div></div>
        <div class="r"><label>Buffer</label><div class="r__c"><SegmentedControl items={['64', '128', '256', '512']} activeIndex={1} /></div></div>
        <div class="live"><span>Output level</span><LevelMeter left={0.55} right={0.5} label="" /><Button label="Test tone" size="small" /></div>
        <p class="n">Changing rate or buffer stops playback for a moment. The player stays where it is.</p>
      </div>
    </section>

    <section class="s">
      <header><Ico name="chevron-down" size={13} /><b>Analysis</b><span>Running 120 / 340</span></header>
      <div class="s__b">
        <div class="r"><label>Analyze new tracks on import</label><div class="r__c r__c--r"><Vanilla make={() => createToggle({ on: true })} /></div></div>
        <div class="r"><label>Engine</label><div class="r__c"><SegmentedControl items={['Built-in', 'Python']} activeIndex={0} /></div></div>
        <div class="r"><label>Workers</label><div class="r__c"><Dropdown options={WORKERS} value="auto" /></div></div>
        <div class="r"><label>Re-analyze library</label><div class="r__c r__c--r"><Button label="Re-analyze…" size="small" /></div></div>
      </div>
    </section>

    <section class="s s--c">
      <header><Ico name="chevron-right" size={13} /><b>Library</b><span>30,412 tracks · library.db</span></header>
    </section>
    <section class="s s--c">
      <header><Ico name="chevron-right" size={13} /><b>Keyboard</b><span>1 changed</span><button class="map">Open keyboard map</button></header>
    </section>
    <section class="s s--c">
      <header><Ico name="chevron-right" size={13} /><b>Advanced</b><span>Reset library, diagnostics, version 0.9.3</span></header>
    </section>
  </div>
</div>

<style>
  .p { flex: none; height: 100%; display: flex; flex-direction: column; background: var(--ff-surface); border-left: 1px solid var(--ff-border); font-family: var(--ff-font); color: var(--ff-text); }
  .p__head { flex: none; display: flex; align-items: center; gap: 8px; height: 44px; padding: 0 10px 0 14px; border-bottom: 1px solid var(--ff-border); }
  .p__t { font-size: 13px; font-weight: 500; }
  .p__f { flex: 1 1 0; min-width: 0; }
  .p__x { border: none; background: none; color: var(--ff-text-mid); display: flex; padding: 5px; }
  .p__scroll { flex: 1 1 0; min-height: 0; overflow: hidden; }
  .s { border-bottom: 1px solid var(--ff-border); }
  .s header { display: flex; align-items: center; gap: 8px; height: 38px; padding: 0 14px; color: var(--ff-text-mid); }
  .s header b { font-size: 13px; font-weight: 500; color: var(--ff-text); }
  .s header span { margin-left: auto; font-size: 11px; color: var(--ff-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .s--c header span { flex: 1; text-align: right; }
  .s header .map { margin-left: 8px; border: 1px solid var(--ff-border-hi); background: none; color: var(--ff-text-mid); border-radius: var(--ff-radius-md); font-size: 11px; height: 22px; padding: 0 8px; font-family: var(--ff-font); }
  .s__b { padding: 2px 14px 14px 35px; display: flex; flex-direction: column; gap: 6px; }
  .r { display: grid; grid-template-columns: 112px minmax(0, 1fr); align-items: center; gap: 8px; min-height: 30px; font-size: 12px; color: var(--ff-text-mid); }
  .r label { line-height: 1.25; }
  .r__c { display: flex; min-width: 0; }
  .r__c :global(.ff-dropdown) { width: 100%; }
  .r__c--r { justify-content: flex-end; }
  .live { display: flex; align-items: center; gap: 10px; padding: 8px 10px; border: 1px solid var(--ff-border); border-radius: var(--ff-radius-lg); background: var(--ff-bg); font-size: 11.5px; color: var(--ff-muted); }
  .live :global(.ff-meter) { flex: 1; }
  .n { margin: 0; font-size: 11px; line-height: 1.45; color: var(--ff-muted); }
</style>
