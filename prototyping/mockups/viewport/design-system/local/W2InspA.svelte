<script>
  import { GhostField, LockMark, ScrubValue, Button, bpmScrub, keyScrub, keyColor } from '../../../../viewport/design-system/svelte.js';
  import { createStars, createColorPicker } from '../../../../viewport/design-system/index.js';
  import Vanilla from './Vanilla.svelte';
  import Ico from './Ico.svelte';
  import { T, ALL_CUES } from './w2-data.js';

  let { width = 340 } = $props();
  const noop = () => {};
</script>

<div class="a" style="width:{width}px">
  <div class="a__scroll">
    <div class="a__id">
      <div class="a__cover" style="background-image:url({T.cover})"><span>Replace</span></div>
      <div class="a__idf">
        <GhostField title value={T.title} label="Title" />
        <GhostField value={T.artist} label="Artist" />
      </div>
    </div>

    <div class="a__sec"><span>TAGS</span><em>edits stay in the library</em></div>
    <div class="a__grid">
      <label>Album</label><GhostField value={T.album} label="Album" />
      <label>Genre</label>
      <div class="a__pair">
        <GhostField value={T.genre} state="edit" suggest={['Melodic Techno', 'Melodic House', 'Minimal / Deep Tech']} label="Genre" />
        <GhostField value={String(T.year)} mono align="right" label="Year" />
      </div>
      <label>Label</label><GhostField value={T.label} state="hover" label="Label" />
      <label>Rating</label>
      <div class="a__pair a__pair--c">
        <Vanilla make={() => createStars({ rating: T.rating, interactive: true })} />
        <Vanilla make={() => createColorPicker({ selected: 2 })} />
      </div>
      <label class="a__top">Comment</label><GhostField multiline value={T.comment} label="Comment" placeholder="Add a note" />
    </div>

    <div class="a__sec a__sec--gap"><span>TEMPO &amp; KEY</span><em>hand-set values survive re-analysis</em></div>
    <div class="a__tempo">
      <div class="a__cell">
        <div class="a__cap"><span>BPM</span><LockMark locked label="Yours" /></div>
        <div class="a__val"><ScrubValue model={bpmScrub} value={125.5} label="BPM" onCommit={noop} /></div>
        <div class="a__was">Analysed 125.00 <button>Use</button></div>
      </div>
      <div class="a__cell">
        <div class="a__cap"><span>KEY</span><span class="a__auto">Analysed</span></div>
        <div class="a__val"><ScrubValue model={keyScrub} value="8A" label="Key" color={keyColor('8A')} onCommit={noop} /></div>
        <div class="a__was">A minor</div>
      </div>
    </div>

    <div class="a__sec a__sec--gap"><span>BEAT GRID</span><span class="a__auto">Analysed</span></div>
    <div class="a__actions">
      <Button label="Shift ‹" size="small" />
      <Button label="Shift ›" size="small" />
      <Button label="Fit to 125.50" size="small" />
      <Button label="Re-detect" size="small" />
    </div>

    <div class="a__sec a__sec--gap"><span>CUES · 4 HOT · 1 MEMORY</span><LockMark locked label="Yours" /></div>
    <div class="a__cues">
      {#each ALL_CUES as c (c.key)}
        <div class="a__cue">
          <span class="a__badge" style="background:{c.color}">{c.letter}</span>
          <GhostField value={c.name} placeholder="Name" label="Cue name" state={c.key === 'h3' ? 'hover' : 'rest'} />
          <GhostField value={c.time} mono align="right" label="Cue time" />
          <button class="a__more" aria-label="Cue menu">···</button>
        </div>
      {/each}
      <button class="a__add"><Ico name="plus" size={12} /> Add at playhead</button>
    </div>

    <div class="a__sec a__sec--gap"><span>FILE</span></div>
    <div class="a__file">…/Kiln/Afterimage EP/01 Afterimage (Extended Mix).flac</div>
  </div>
  <div class="a__fade"></div>
  <div class="a__bar"><i></i></div>
</div>

<style>
  .a { position: relative; height: 100%; overflow: hidden; background: var(--ff-surface); border-left: 1px solid var(--ff-border); font-family: var(--ff-font); color: var(--ff-text); }
  .a__scroll { height: 100%; overflow: hidden; padding: 12px 12px 40px; display: flex; flex-direction: column; gap: 4px; }
  .a__fade { position: absolute; left: 0; right: 0; bottom: 0; height: 56px; background: linear-gradient(to bottom, transparent, var(--ff-surface)); pointer-events: none; }
  .a__bar { position: absolute; right: 2px; top: 10px; bottom: 10px; width: 4px; }
  .a__bar i { display: block; height: 62%; border-radius: 2px; background: var(--ff-border-hi); }
  .a__id { display: flex; gap: 10px; margin-bottom: 6px; }
  .a__cover { position: relative; flex: none; width: 60px; height: 60px; border-radius: var(--ff-radius-xl); background-size: cover; border: 1px solid var(--ff-border-hi); overflow: hidden; }
  .a__cover span { position: absolute; inset: auto 0 0 0; padding: 2px 0; text-align: center; font-size: 10px; background: rgba(0, 0, 0, 0.6); color: var(--ff-text-mid); }
  .a__idf { flex: 1 1 0; min-width: 0; display: flex; flex-direction: column; gap: 2px; justify-content: center; }
  .a__sec { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin-top: 8px; font-family: var(--ff-font-mono); font-size: var(--ff-type-label); letter-spacing: 0.06em; color: var(--ff-muted); }
  .a__sec em { font-style: normal; letter-spacing: 0; color: var(--ff-faint); font-family: var(--ff-font); font-size: 10.5px; }
  .a__sec--gap { margin-top: 12px; padding-top: 10px; border-top: 1px solid var(--ff-border); }
  .a__grid { display: grid; grid-template-columns: 54px minmax(0, 1fr); gap: 3px 6px; align-items: center; }
  .a__grid label { font-size: 11px; color: var(--ff-muted); padding-left: 2px; }
  .a__top { align-self: start; padding-top: 7px; }
  .a__pair { display: grid; grid-template-columns: minmax(0, 1fr) 62px; gap: 4px; }
  .a__pair--c { grid-template-columns: auto 1fr; gap: 14px; align-items: center; padding-left: 8px; height: 26px; }
  .a__tempo { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
  .a__cell { padding: 8px 10px; border: 1px solid var(--ff-border); border-radius: var(--ff-radius-lg); background: var(--ff-bg); display: flex; flex-direction: column; gap: 4px; }
  .a__cap { display: flex; align-items: center; justify-content: space-between; font-family: var(--ff-font-mono); font-size: var(--ff-type-label-sm); letter-spacing: 0.06em; color: var(--ff-muted); height: 18px; }
  .a__auto { font-family: var(--ff-font-mono); font-size: var(--ff-type-label-sm); letter-spacing: 0.06em; text-transform: uppercase; color: var(--ff-faint); }
  .a__val { font-family: var(--ff-font-mono); font-size: 20px; font-weight: 500; padding-left: 5px; }
  .a__was { font-size: 10.5px; color: var(--ff-muted); display: flex; gap: 6px; align-items: center; }
  .a__was button { border: none; background: none; padding: 0; color: var(--ff-text-mid); font-size: 10.5px; font-family: var(--ff-font); text-decoration: underline; text-underline-offset: 2px; }
  .a__actions { display: flex; gap: 6px; flex-wrap: wrap; }
  .a__cues { display: flex; flex-direction: column; gap: 1px; }
  .a__cue { display: grid; grid-template-columns: 20px minmax(0, 1fr) 54px 22px; align-items: center; gap: 6px; }
  .a__badge { width: 18px; height: 18px; border-radius: var(--ff-radius-sm); color: #0a0a0a; font-family: var(--ff-font-mono); font-size: 10px; font-weight: 600; display: flex; align-items: center; justify-content: center; }
  .a__more { border: none; background: none; color: var(--ff-muted); font-size: 12px; padding: 0; }
  .a__add { align-self: flex-start; display: flex; align-items: center; gap: 6px; margin-top: 4px; height: 24px; padding: 0 8px; border: 1px dashed var(--ff-border-hi); background: none; border-radius: var(--ff-radius-md); color: var(--ff-text-mid); font-size: 11.5px; font-family: var(--ff-font); }
  .a__file { font-family: var(--ff-font-mono); font-size: 11px; color: var(--ff-muted); padding: 2px 2px; }
</style>
