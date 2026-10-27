<script>
  import { GhostField, LockMark, ScrubValue, bpmScrub, keyScrub, keyColor } from '../../../../viewport/design-system/svelte.js';
  import Ico from './Ico.svelte';
  import W3Cues from './W3Cues.svelte';
  import { T } from './w2-data.js';

  let { width = 340 } = $props();
  const noop = () => {};
</script>

<div class="b" style="width:{width}px">
  <div class="b__hero" style="background-image:url({T.cover})">
    <div class="b__shade"></div>
    <div class="b__id">
      <GhostField title value={T.title} label="Title" />
      <GhostField value={T.artist} label="Artist" />
    </div>
  </div>

  <div class="b__ess">
    <div class="b__fig"><ScrubValue model={bpmScrub} value={125.5} label="BPM" onCommit={noop} /></div>
    <div class="b__fig"><ScrubValue model={keyScrub} value="8A" label="Key" color={keyColor('8A')} onCommit={noop} /></div>
    <span class="b__len">{T.time}</span>
    <LockMark locked title="BPM and grid are hand-set. Re-analysis keeps them." />
  </div>

  <div class="b__fold b__fold--open">
    <button class="b__fh"><Ico name="chevron-down" size={12} />Cues <em>5</em></button>
    <div class="b__fb"><W3Cues /></div>
  </div>

  <div class="b__fold">
    <button class="b__fh"><Ico name="chevron-right" size={12} />Details <span class="b__sum">{T.genre} · {T.label} · {T.year}</span></button>
  </div>

  <div class="b__fold">
    <button class="b__fh"><Ico name="chevron-right" size={12} />File <span class="b__sum">FLAC · 24 bit · 46 MB</span></button>
  </div>
</div>

<style>
  .b { flex: none; height: 100%; overflow: hidden; box-sizing: border-box; display: flex; flex-direction: column; background: var(--ff-surface); border-left: 1px solid var(--ff-border); font-family: var(--ff-font); color: var(--ff-text); }
  .b__hero { position: relative; flex: none; height: 168px; background-size: cover; background-position: center; }
  .b__shade { position: absolute; inset: 0; background: linear-gradient(to bottom, rgba(0, 0, 0, 0.05) 30%, rgba(0, 0, 0, 0.82)); }
  .b__id { position: absolute; left: 8px; right: 8px; bottom: 10px; display: flex; flex-direction: column; gap: 1px; }
  .b__id :global(.ff-ghost__input) { font-size: 18px; font-weight: 500; }
  .b__ess { display: flex; align-items: center; gap: 10px; padding: 18px 14px; }
  .b__fig { flex: 1 1 0; min-width: 0; display: flex; }
  .b__len { font-family: var(--ff-font-mono); font-size: 14px; color: var(--ff-text-mid); padding: 0 4px; }
  .b__fold { padding: 0 8px; }
  .b__fold--open { padding-bottom: 12px; }
  .b__fh { display: flex; align-items: center; gap: 6px; width: 100%; height: 34px; padding: 0 6px; border: none; background: none; color: var(--ff-text-mid); font-family: var(--ff-font); font-size: 12.5px; text-align: left; }
  .b__fh em { font-style: normal; color: var(--ff-faint); }
  .b__sum { margin-left: auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 11.5px; color: var(--ff-faint); }
  .b__fb { padding: 0 0 0 4px; }
</style>
