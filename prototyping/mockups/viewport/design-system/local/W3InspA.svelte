<script>
  import { GhostField, LockMark, ScrubValue, bpmScrub, keyScrub, keyColor } from '../../../../viewport/design-system/svelte.js';
  import { createStars } from '../../../../viewport/design-system/index.js';
  import Vanilla from './Vanilla.svelte';
  import W3Cues from './W3Cues.svelte';
  import { T, FILE_PATH } from './w2-data.js';

  let { width = 340 } = $props();
  const noop = () => {};
</script>

<div class="a" style="width:{width}px">
  <div class="a__id">
    <div class="a__cover" style="background-image:url({T.cover})"></div>
    <div class="a__idf">
      <GhostField title value={T.title} label="Title" />
      <GhostField value={T.artist} label="Artist" />
    </div>
  </div>

  <div class="a__ess">
    <div class="a__fig"><span>BPM</span><div class="a__sv"><ScrubValue model={bpmScrub} value={125.5} label="BPM" onCommit={noop} /></div></div>
    <div class="a__fig"><span>KEY</span><div class="a__sv"><ScrubValue model={keyScrub} value="8A" label="Key" color={keyColor('8A')} onCommit={noop} /></div></div>
    <div class="a__fig a__fig--len"><span>LENGTH</span><b>{T.time}</b></div>
  </div>
  <div class="a__lock"><LockMark locked title="BPM and grid are hand-set. Re-analysis keeps them." /><span>BPM and grid are hand-set</span></div>

  <div class="a__rows">
    <label>Album</label><GhostField value={T.album} label="Album" />
    <label>Genre</label><GhostField value={T.genre} label="Genre" />
    <label>Label</label><GhostField value={T.label} label="Label" state="hover" />
    <label>Year</label><GhostField value={String(T.year)} mono label="Year" />
    <label>Rating</label><div class="a__in"><Vanilla make={() => createStars({ rating: T.rating, interactive: true })} /></div>
    <label>Colour</label><div class="a__in a__col"><i></i>Amber</div>
    <label class="a__top">Comment</label><GhostField multiline value={T.comment} label="Comment" placeholder="Add a note" />
  </div>

  <div class="a__h">Cues <em>4 hot · 1 memory</em></div>
  <W3Cues />

  <div class="a__file">{FILE_PATH}</div>
</div>

<style>
  .a { flex: none; height: 100%; overflow: hidden; box-sizing: border-box; padding: 20px 14px 14px; display: flex; flex-direction: column; background: var(--ff-surface); border-left: 1px solid var(--ff-border); font-family: var(--ff-font); color: var(--ff-text); }
  .a__id { display: flex; gap: 12px; align-items: center; }
  .a__cover { flex: none; width: 88px; height: 88px; border-radius: var(--ff-radius-xl); background-size: cover; }
  .a__idf { flex: 1 1 0; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .a__idf :global(.ff-ghost__input) { font-size: 15px; font-weight: 500; }
  .a__ess { display: flex; gap: 8px; align-items: flex-start; margin-top: 24px; }
  .a__fig { display: flex; flex-direction: column; gap: 4px; flex: 1 1 0; min-width: 0; }
  .a__fig span { font-family: var(--ff-font-mono); font-size: var(--ff-type-label-sm); letter-spacing: 0.06em; color: var(--ff-faint); }
  .a__fig b { font-family: var(--ff-font-mono); font-size: 15px; font-weight: 500; line-height: 30px; color: var(--ff-text-mid); }
  .a__sv { display: flex; min-width: 0; }
  .a__fig--len { flex: none; width: 48px; }
  .a__lock { margin-top: 10px; padding-left: 2px; display: flex; align-items: center; gap: 8px; font-size: 11px; color: var(--ff-faint); }
  .a__rows { margin-top: 24px; display: grid; grid-template-columns: 52px minmax(0, 1fr); gap: 2px 6px; align-items: center; }
  .a__rows label { font-size: 11px; color: var(--ff-faint); padding-left: 2px; }
  .a__top { align-self: start; padding-top: 7px; }
  .a__in { padding-left: 8px; height: 26px; display: flex; align-items: center; }
  .a__col { gap: 8px; font-size: 12.5px; color: var(--ff-text-mid); }
  .a__col i { width: 10px; height: 10px; border-radius: 3px; background: #f59e0b; }
  .a__h { margin: 24px 0 6px 2px; font-size: 11px; color: var(--ff-faint); }
  .a__h em { font-style: normal; margin-left: 6px; opacity: 0.7; }
  .a__file { margin-top: auto; padding: 14px 2px 0; font-family: var(--ff-font-mono); font-size: 10.5px; color: var(--ff-faint); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
