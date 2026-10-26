<script>
  import { GhostField, LockMark, ScrubValue, SegmentedControl, Button, bpmScrub, keyScrub, keyColor } from '../../../../viewport/design-system/svelte.js';
  import { createStars, createColorPicker } from '../../../../viewport/design-system/index.js';
  import Vanilla from './Vanilla.svelte';
  import Ico from './Ico.svelte';
  import W2Wave from './W2Wave.svelte';
  import { T, HOT, MEMORY, FILE_PATH } from './w2-data.js';

  let { width = 380, tab = 'prep' } = $props();
  const noop = () => {};
  const sel = HOT[2];
</script>

<div class="c" style="width:{width}px">
  <div class="c__seg">
    <SegmentedControl items={['Prep', 'Tags']} activeIndex={tab === 'prep' ? 0 : 1} />
    <span class="c__hint">{tab === 'prep' ? 'tempo, grid, cues' : 'text, rating, colour, file'}</span>
  </div>

  {#if tab === 'prep'}
    <div class="c__body">
      <div class="c__title"><b>{T.title}</b><span>{T.artist}</span></div>

      <W2Wave width={width - 24} height={30} win />
      <div class="c__zoom"><W2Wave width={width - 24} height={70} zoom selectedKey={sel.key} tip="C · 3:52.00 · bar 117" /></div>

      <div class="c__chips">
        {#each [...HOT, ...MEMORY] as k (k.key)}
          <span class="c__chip" class:c__chip--on={k.key === sel.key}><i style="background:{k.color}">{k.letter}</i>{k.time}</span>
        {/each}
        <span class="c__chip c__chip--add">+ at playhead</span>
        <span class="c__lk"><LockMark locked label="Yours" /></span>
      </div>

      <div class="c__cue">
        <span class="c__badge" style="background:{sel.color}">{sel.letter}</span>
        <GhostField value={sel.name} placeholder="Name" label="Cue name" />
        <GhostField value="3:52.00" mono align="right" label="Cue time" />
        <div class="c__cueb">
          <Button label="‹ beat" size="small" />
          <Button label="beat ›" size="small" />
          <Button label="To playhead" size="small" />
          <button class="c__ico" aria-label="Delete cue"><Ico name="trash" size={13} /></button>
        </div>
      </div>

      <div class="c__sec"><span>TEMPO &amp; KEY</span></div>
      <div class="c__tempo">
        <div class="c__cell c__cell--own">
          <div class="c__cap"><span>BPM</span><LockMark locked label="Yours" /></div>
          <div class="c__big"><ScrubValue model={bpmScrub} value={125.5} label="BPM" onCommit={noop} /></div>
          <div class="c__cmp"><span>Analysed</span><b>125.00</b><button>Use</button></div>
        </div>
        <div class="c__cell">
          <div class="c__cap"><span>KEY</span><LockMark locked={false} label="Analysed" /></div>
          <div class="c__big"><ScrubValue model={keyScrub} value="8A" label="Key" color={keyColor('8A')} onCommit={noop} /></div>
          <div class="c__cmp"><span>A minor</span></div>
        </div>
      </div>

      <div class="c__sec"><span>BEAT GRID</span><LockMark locked={false} label="Analysed" /></div>
      <div class="c__grid">
        <Button label="Shift ‹" size="small" />
        <Button label="Shift ›" size="small" />
        <Button label="×2" size="small" />
        <Button label="½" size="small" />
        <Button label="Downbeat here" size="small" />
        <Button label="Fit to 125.50" size="small" />
      </div>
      <p class="c__note">Hand-set values stay put when the library is re-analysed. Use analysed hands one back.</p>
    </div>
  {:else}
    <div class="c__body">
      <div class="c__id">
        <div class="c__cover" style="background-image:url({T.cover})"></div>
        <div class="c__idf">
          <GhostField title value={T.title} label="Title" />
          <GhostField value={T.artist} label="Artist" />
          <GhostField value={T.album} label="Album" />
        </div>
      </div>
      <div class="c__form">
        <label>Genre</label><GhostField value={T.genre} label="Genre" />
        <label>Label</label><GhostField value={T.label} label="Label" />
        <label>Year</label><GhostField value={String(T.year)} mono label="Year" />
        <label>Rating</label><div class="c__rowc"><Vanilla make={() => createStars({ rating: T.rating, interactive: true })} /></div>
        <label>Colour</label><div class="c__rowc"><Vanilla make={() => createColorPicker({ selected: 2 })} /></div>
        <label class="c__top">Comment</label><GhostField multiline value={T.comment} label="Comment" />
      </div>
      <div class="c__sec"><span>FILE</span><span class="c__ro">read-only</span></div>
      <div class="c__file">
        <div class="c__path">{FILE_PATH}</div>
        <div class="c__facts"><span>FLAC</span><span>44.1 kHz</span><span>1,411 kbps</span><span>8:11</span><span>54.2 MB</span></div>
        <div class="c__facts c__facts--b"><button>Reveal in Finder</button><button>Locate…</button><button>Re-analyse</button></div>
      </div>
      <p class="c__note">Tags are saved to the library only. The audio file is never touched.</p>
    </div>
  {/if}
</div>

<style>
  .c { position: relative; height: 100%; overflow: hidden; background: var(--ff-surface); border-left: 1px solid var(--ff-border); font-family: var(--ff-font); color: var(--ff-text); display: flex; flex-direction: column; }
  .c__seg { flex: none; display: flex; align-items: center; justify-content: space-between; gap: 10px; padding: 10px 12px; border-bottom: 1px solid var(--ff-border); }
  .c__hint { font-size: 11px; color: var(--ff-muted); }
  .c__body { flex: 1 1 0; min-height: 0; overflow: hidden; padding: 12px; display: flex; flex-direction: column; gap: 8px; }
  .c__title { display: flex; align-items: baseline; gap: 8px; min-width: 0; }
  .c__title b { font-size: 14px; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .c__title span { font-size: 12px; color: var(--ff-muted); white-space: nowrap; }
  .c__zoom { margin-top: 18px; }
  .c__chips { display: flex; align-items: center; flex-wrap: wrap; gap: 4px; margin-top: 8px; }
  .c__lk { margin-left: auto; }
  .c__chip { display: inline-flex; align-items: center; gap: 5px; height: 22px; padding: 0 7px 0 3px; border: 1px solid var(--ff-border); border-radius: var(--ff-radius-md); background: var(--ff-bg); font-family: var(--ff-font-mono); font-size: 10.5px; color: var(--ff-text-mid); }
  .c__chip i { font-style: normal; width: 15px; height: 15px; border-radius: 3px; display: flex; align-items: center; justify-content: center; color: #0a0a0a; font-weight: 700; font-size: 10px; }
  .c__chip--on { border-color: rgba(255, 255, 255, 0.34); background: var(--ff-elev); color: var(--ff-text); }
  .c__chip--add { padding: 0 8px; border-style: dashed; color: var(--ff-muted); font-family: var(--ff-font); }
  .c__cue { display: grid; grid-template-columns: 20px minmax(0, 1fr) 70px; gap: 4px 6px; align-items: center; padding: 8px; border: 1px solid var(--ff-border-hi); border-radius: var(--ff-radius-lg); background: var(--ff-bg); }
  .c__badge { width: 18px; height: 18px; border-radius: 3px; color: #0a0a0a; font-weight: 700; font-size: 11px; font-family: var(--ff-font-mono); display: flex; align-items: center; justify-content: center; }
  .c__cueb { grid-column: 1 / -1; display: flex; gap: 4px; align-items: center; }
  .c__ico { margin-left: auto; width: 26px; height: 24px; border: 1px solid var(--ff-border); background: none; border-radius: var(--ff-radius-md); color: var(--ff-text-mid); display: flex; align-items: center; justify-content: center; }
  .c__sec { display: flex; align-items: center; justify-content: space-between; margin-top: 6px; padding-top: 8px; border-top: 1px solid var(--ff-border); font-family: var(--ff-font-mono); font-size: var(--ff-type-label); letter-spacing: 0.06em; color: var(--ff-muted); }
  .c__ro { font-family: var(--ff-font); letter-spacing: 0; color: var(--ff-faint); font-size: 10.5px; }
  .c__tempo { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
  .c__cell { padding: 8px 10px; border: 1px solid var(--ff-border); border-radius: var(--ff-radius-lg); background: var(--ff-bg); display: flex; flex-direction: column; gap: 3px; }
  .c__cell--own { border-color: var(--ff-border-hi); }
  .c__cap { display: flex; align-items: center; justify-content: space-between; height: 18px; font-family: var(--ff-font-mono); font-size: var(--ff-type-label-sm); letter-spacing: 0.06em; color: var(--ff-muted); }
  .c__big { font-family: var(--ff-font-mono); font-size: 22px; font-weight: 500; padding-left: 5px; height: 30px; display: flex; align-items: center; }
  .c__cmp { display: flex; gap: 6px; align-items: baseline; font-size: 10.5px; color: var(--ff-muted); }
  .c__cmp b { font-family: var(--ff-font-mono); font-weight: 500; color: var(--ff-text-mid); }
  .c__cmp button { margin-left: auto; border: none; background: none; padding: 0; color: var(--ff-text-mid); font-size: 10.5px; text-decoration: underline; text-underline-offset: 2px; font-family: var(--ff-font); }
  .c__grid { display: flex; flex-wrap: wrap; gap: 6px; }
  .c__note { margin: 2px 0 0; font-size: 11px; line-height: 1.45; color: var(--ff-muted); }
  .c__id { display: flex; gap: 12px; }
  .c__cover { flex: none; width: 92px; height: 92px; border-radius: var(--ff-radius-xl); background-size: cover; border: 1px solid var(--ff-border-hi); }
  .c__idf { flex: 1 1 0; min-width: 0; display: flex; flex-direction: column; gap: 2px; justify-content: center; }
  .c__form { display: grid; grid-template-columns: 56px minmax(0, 1fr); gap: 4px 6px; align-items: center; }
  .c__form label { font-size: 11px; color: var(--ff-muted); padding-left: 2px; }
  .c__top { align-self: start; padding-top: 7px; }
  .c__rowc { padding-left: 8px; height: 26px; display: flex; align-items: center; }
  .c__file { display: flex; flex-direction: column; gap: 6px; }
  .c__path { font-family: var(--ff-font-mono); font-size: 11px; color: var(--ff-text-mid); word-break: break-all; line-height: 1.45; }
  .c__facts { display: flex; gap: 10px; font-family: var(--ff-font-mono); font-size: 11px; color: var(--ff-muted); }
  .c__facts--b { gap: 6px; }
  .c__facts button { height: 24px; padding: 0 8px; border: 1px solid var(--ff-border-hi); background: none; border-radius: var(--ff-radius-md); color: var(--ff-text-mid); font-size: 11px; font-family: var(--ff-font); }
</style>
