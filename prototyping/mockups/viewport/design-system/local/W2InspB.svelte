<script>
  import { GhostField, LockMark, ScrubValue, CueEditor, bpmScrub, keyColor } from '../../../../viewport/design-system/svelte.js';
  import { createStars, createColorPicker } from '../../../../viewport/design-system/index.js';
  import Vanilla from './Vanilla.svelte';
  import Ico from './Ico.svelte';
  import KeyWheel from './KeyWheel.svelte';
  import W2Wave from './W2Wave.svelte';
  import { T, HOT, MEMORY, CUE_PALETTE } from './w2-data.js';

  let { width = 340, popover = 'none' } = $props();
  const noop = () => {};
  const editing = HOT[2];
</script>

<div class="b" style="width:{width}px">
  <div class="b__scroll">
    <div class="b__id">
      <div class="b__cover" style="background-image:url({T.cover})"></div>
      <div class="b__idf">
        <GhostField title value={T.title} label="Title" />
        <GhostField value={T.artist} label="Artist" />
        <GhostField value={T.album} label="Album" />
      </div>
    </div>

    <W2Wave width={width - 24} height={40} grid={false} />

    <div class="b__tiles">
      <div class="b__tile b__tile--own">
        <div class="b__cap"><span>BPM</span><LockMark locked /></div>
        <div class="b__big"><ScrubValue model={bpmScrub} value={125.5} label="BPM" onCommit={noop} /></div>
        <div class="b__sub">analysed 125.00</div>
      </div>
      <div class="b__tile" class:b__tile--open={popover === 'key'}>
        <div class="b__cap"><span>KEY</span><LockMark locked={false} /></div>
        <div class="b__big b__big--btn" style="color:{keyColor('8A')}">8A <Ico name="chevron-down" size={12} /></div>
        <div class="b__sub">A minor</div>
      </div>
      <div class="b__tile" class:b__tile--open={popover === 'grid'}>
        <div class="b__cap"><span>GRID</span><LockMark locked={false} /></div>
        <div class="b__big b__big--btn b__big--sm">Edit <Ico name="chevron-down" size={12} /></div>
        <div class="b__sub">125.00 BPM</div>
      </div>
      <div class="b__tile b__tile--flat">
        <div class="b__cap"><span>RATING</span></div>
        <div class="b__row"><Vanilla make={() => createStars({ rating: T.rating, interactive: true })} /></div>
      </div>
      <div class="b__tile b__tile--flat">
        <div class="b__cap"><span>COLOUR</span></div>
        <div class="b__row"><Vanilla make={() => createColorPicker({ selected: 2 })} /></div>
      </div>
      <div class="b__tile b__tile--flat b__tile--ro">
        <div class="b__cap"><span>TIME</span></div>
        <div class="b__row b__mono">8:11 · FLAC</div>
      </div>
    </div>

    <div class="b__fields">
      <div class="b__pair"><GhostField value={T.genre} placeholder="Genre" label="Genre" /><GhostField value={T.label} placeholder="Label" label="Label" /><GhostField value={String(T.year)} mono align="right" label="Year" /></div>
      <GhostField multiline value={T.comment} placeholder="Add a note" label="Comment" />
    </div>

    <div class="b__sec"><span>CUES</span><LockMark locked label="Yours" /></div>
    <div class="b__chips">
      {#each [...HOT, ...MEMORY] as c (c.key)}
        <span class="b__chip" class:b__chip--on={popover === 'cue' && c.key === editing.key}><i style="background:{c.color}">{c.letter}</i>{c.time}</span>
      {/each}
      <span class="b__chip b__chip--add">+</span>
    </div>
    {#if popover === 'cue'}
      <div class="b__cued">
        <CueEditor chip={editing} palette={CUE_PALETTE} />
        <div class="b__time">
          <span>Time</span><GhostField mono value="3:52.00" label="Cue time" state="edit" />
          <button>‹ beat</button><button>beat ›</button><button>Playhead</button>
        </div>
      </div>
    {/if}
  </div>

  {#if popover === 'key'}
    <div class="b__pop b__pop--key">
      <div class="b__poph"><b>Key</b><span>click a key · ↑↓ steps</span></div>
      <div class="b__wheel"><KeyWheel selected="8A" size={196} /></div>
      <div class="b__popf"><span>Analysed 8A</span><button>Keep analysed</button></div>
    </div>
  {:else if popover === 'grid'}
    <div class="b__pop b__pop--grid">
      <button>Shift grid ‹ 10 ms</button>
      <button>Shift grid › 10 ms</button>
      <button>Double tempo (×2)</button>
      <button>Half tempo (½)</button>
      <button>Fit grid to 125.50 BPM</button>
      <button>Set downbeat at playhead</button>
      <hr />
      <button>Re-detect grid</button>
    </div>
  {/if}
</div>

<style>
  .b { position: relative; height: 100%; overflow: hidden; background: var(--ff-surface); border-left: 1px solid var(--ff-border); font-family: var(--ff-font); color: var(--ff-text); }
  .b__scroll { height: 100%; overflow: hidden; padding: 12px; display: flex; flex-direction: column; gap: 8px; }
  .b__id { display: flex; gap: 10px; }
  .b__cover { flex: none; width: 76px; height: 76px; border-radius: var(--ff-radius-xl); background-size: cover; border: 1px solid var(--ff-border-hi); }
  .b__idf { flex: 1 1 0; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .b__tiles { display: grid; grid-template-columns: repeat(3, 1fr); gap: 6px; }
  .b__tile { position: relative; padding: 7px 8px 8px; border: 1px solid var(--ff-border); border-radius: var(--ff-radius-lg); background: var(--ff-bg); display: flex; flex-direction: column; gap: 3px; min-width: 0; }
  .b__tile--own { border-color: var(--ff-border-hi); }
  .b__tile--open { border-color: rgba(255, 255, 255, 0.34); background: var(--ff-elev); }
  .b__tile--flat { background: transparent; }
  .b__tile--ro { border-style: dashed; }
  .b__cap { display: flex; align-items: center; justify-content: space-between; height: 18px; font-family: var(--ff-font-mono); font-size: var(--ff-type-label-sm); letter-spacing: 0.06em; color: var(--ff-muted); }
  .b__big { font-family: var(--ff-font-mono); font-size: 19px; font-weight: 500; padding-left: 5px; height: 26px; display: flex; align-items: center; }
  .b__big--btn { gap: 4px; cursor: pointer; }
  .b__big--sm { font-size: 13px; color: var(--ff-text-mid); font-family: var(--ff-font); }
  .b__sub { font-size: 10.5px; color: var(--ff-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .b__row { height: 26px; display: flex; align-items: center; }
  .b__mono { font-family: var(--ff-font-mono); font-size: 11.5px; color: var(--ff-text-mid); }
  .b__fields { display: flex; flex-direction: column; gap: 3px; }
  .b__pair { display: grid; grid-template-columns: 1fr 1fr 54px; gap: 3px; }
  .b__sec { display: flex; align-items: center; justify-content: space-between; margin-top: 4px; font-family: var(--ff-font-mono); font-size: var(--ff-type-label); letter-spacing: 0.06em; color: var(--ff-muted); }
  .b__chips { display: flex; flex-wrap: wrap; gap: 4px; }
  .b__chip { display: inline-flex; align-items: center; gap: 6px; height: 24px; padding: 0 8px 0 4px; border: 1px solid var(--ff-border); border-radius: var(--ff-radius-md); background: var(--ff-bg); font-family: var(--ff-font-mono); font-size: 11px; color: var(--ff-text-mid); }
  .b__chip i { font-style: normal; width: 16px; height: 16px; border-radius: 3px; display: flex; align-items: center; justify-content: center; color: #0a0a0a; font-weight: 700; font-size: 10px; }
  .b__chip--on { border-color: rgba(255, 255, 255, 0.34); background: var(--ff-elev); }
  .b__chip--add { padding: 0 10px; border-style: dashed; color: var(--ff-muted); }
  .b__cued { display: flex; flex-direction: column; gap: 6px; }
  .b__time { display: grid; grid-template-columns: 34px 1fr auto auto auto; gap: 4px; align-items: center; font-size: 11px; color: var(--ff-muted); }
  .b__time button { height: 24px; padding: 0 7px; border: 1px solid var(--ff-border-hi); background: var(--ff-elev); border-radius: var(--ff-radius-md); color: var(--ff-text-mid); font-size: 11px; font-family: var(--ff-font); }
  .b__pop { position: absolute; z-index: 20; background: var(--ff-elev); border: 1px solid var(--ff-border-hi); border-radius: var(--ff-radius-xl); box-shadow: var(--ff-shadow-menu); }
  .b__pop--key { left: 74px; top: 266px; width: 226px; padding: 10px; display: flex; flex-direction: column; gap: 6px; }
  .b__poph { display: flex; justify-content: space-between; align-items: baseline; font-size: 12px; }
  .b__poph span { font-size: 10.5px; color: var(--ff-muted); }
  .b__wheel { display: flex; justify-content: center; }
  .b__popf { display: flex; justify-content: space-between; align-items: center; font-size: 11px; color: var(--ff-muted); padding-top: 4px; border-top: 1px solid var(--ff-border); }
  .b__popf button { border: none; background: none; color: var(--ff-text-mid); text-decoration: underline; text-underline-offset: 2px; font-size: 11px; font-family: var(--ff-font); }
  .b__pop--grid { left: 150px; top: 266px; width: 180px; padding: 4px; display: flex; flex-direction: column; }
  .b__pop--grid button { text-align: left; height: 26px; padding: 0 8px; border: none; background: none; color: var(--ff-text-mid); font-size: 12px; font-family: var(--ff-font); border-radius: var(--ff-radius-sm); }
  .b__pop--grid button:first-child { background: var(--ff-select); color: var(--ff-text); }
  .b__pop--grid hr { border: none; border-top: 1px solid var(--ff-border); margin: 3px 0; width: 100%; }
</style>
