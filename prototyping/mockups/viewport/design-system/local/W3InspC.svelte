<script>
  import { GhostField, LockMark, ScrubValue, bpmScrub, keyScrub, keyColor } from '../../../../viewport/design-system/svelte.js';
  import { createStars } from '../../../../viewport/design-system/index.js';
  import Vanilla from './Vanilla.svelte';
  import W2Wave from './W2Wave.svelte';
  import { T, ALL_CUES } from './w2-data.js';

  let { width = 340 } = $props();
  const noop = () => {};
</script>

<div class="c" style="width:{width}px">
  <div class="c__id">
    <div class="c__cover" style="background-image:url({T.cover})"></div>
    <div class="c__idf">
      <GhostField title value={T.title} label="Title" />
      <GhostField value={T.artist} label="Artist" />
    </div>
  </div>

  <div class="c__ess">
    <div class="c__fig"><ScrubValue model={bpmScrub} value={125.5} label="BPM" onCommit={noop} /></div>
    <div class="c__fig"><ScrubValue model={keyScrub} value="8A" label="Key" color={keyColor('8A')} onCommit={noop} /></div>
    <span class="c__len">{T.time}</span>
  </div>

  <div class="c__cap"><span>Grid and cues</span><LockMark locked title="Grid and cues are hand-set. Re-analysis keeps them." /></div>
  <div class="c__wave"><W2Wave width={width - 28} height={44} /></div>

  <div class="c__list">
    {#each ALL_CUES as c (c.key)}
      <div class="c__cue">
        <span class="c__l" style="color:{c.color}">{c.letter}</span>
        <GhostField value={c.name} placeholder="Unnamed" label="Cue name" state={c.key === 'h3' ? 'hover' : 'rest'} />
        <GhostField value={c.time} mono align="right" label="Cue time" />
      </div>
    {/each}
  </div>

  <div class="c__tags">
    <div class="c__line">
      <span style="width:124px"><GhostField value={T.genre} label="Genre" /></span><i>·</i>
      <span style="width:62px"><GhostField value={T.label} label="Label" /></span><i>·</i>
      <span style="width:50px"><GhostField value={String(T.year)} mono label="Year" /></span>
    </div>
    <GhostField value={T.album} label="Album" />
    <div class="c__line c__line--r">
      <Vanilla make={() => createStars({ rating: T.rating, interactive: true })} />
      <span class="c__dot"></span>
    </div>
    <GhostField multiline value={T.comment} label="Comment" placeholder="Add a note" />
  </div>
</div>

<style>
  .c { flex: none; height: 100%; overflow: hidden; box-sizing: border-box; padding: 16px 14px 14px; display: flex; flex-direction: column; background: var(--ff-surface); border-left: 1px solid var(--ff-border); font-family: var(--ff-font); color: var(--ff-text); }
  .c__id { display: flex; gap: 12px; align-items: center; }
  .c__cover { flex: none; width: 64px; height: 64px; border-radius: var(--ff-radius-xl); background-size: cover; }
  .c__idf { flex: 1 1 0; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .c__idf :global(.ff-ghost__input) { font-size: 16px; font-weight: 500; }
  .c__ess { display: flex; align-items: center; gap: 10px; margin-top: 20px; }
  .c__fig { flex: 1 1 0; min-width: 0; display: flex; }
  .c__len { font-family: var(--ff-font-mono); font-size: 14px; color: var(--ff-text-mid); padding: 0 4px; }
  .c__cap { display: flex; align-items: center; justify-content: space-between; margin: 22px 0 6px 2px; font-size: 11px; color: var(--ff-faint); }
  .c__wave { padding-bottom: 6px; }
  .c__list { display: flex; flex-direction: column; margin-top: 6px; }
  .c__cue { display: grid; grid-template-columns: 16px minmax(0, 1fr) 56px; align-items: center; gap: 4px; }
  .c__l { font-family: var(--ff-font-mono); font-size: 11px; font-weight: 600; text-align: center; }
  .c__tags { margin-top: 22px; display: flex; flex-direction: column; gap: 2px; color: var(--ff-text-mid); }
  .c__line { display: flex; align-items: center; flex-wrap: wrap; }
  .c__line i { font-style: normal; color: var(--ff-faint); }
  .c__line > span { display: block; }
  .c__line--r { gap: 14px; padding-left: 8px; height: 26px; }
  .c__dot { width: 10px; height: 10px; border-radius: 3px; background: #f59e0b; }
</style>
