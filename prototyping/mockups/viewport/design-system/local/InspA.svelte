<script>
  import Ico from './Ico.svelte';
  import Fld from './Fld.svelte';
  import Stars from './Stars.svelte';
  import Swatches from './Swatches.svelte';
  import Scrub from './Scrub.svelte';
  import { SELECTED, CUES } from './data.js';

  let { width = 320, showHeader = true, bpmState = 'drag', genreEdit = true, cueRows = true, headerRight = '' } = $props();
  const t = SELECTED;
</script>

<div class="ia" style="width:{width}px">
  {#if showHeader}
    <div class="ia__head">
      <span class="ia__kicker">TRACK</span>
      <span class="ia__saved"><Ico name="check" size={11} /> Saved</span>
      {#if headerRight}<span class="ia__hr">{headerRight}</span>{/if}
      <button class="ia__ico" aria-label="Track actions"><Ico name="ellipsis" size={14} /></button>
    </div>
  {/if}
  <div class="ia__body">
    <div class="ia__id">
      <div class="ia__cover" style="background-image:url({t.cover})"><span class="ia__swap">Replace</span></div>
      <div class="ia__idf">
        <Fld value={t.title} big />
        <Fld value={t.artist} />
        <Fld value={t.album} state="hover" />
      </div>
    </div>

    <div class="ia__scrubs">
      <Scrub label="BPM" value="125.50" state={bpmState} delta="+0.50" sub="grid locked" />
      <Scrub label="KEY" value="8A" color="var(--ff-key-8)" sub="A min" />
    </div>
    <div class="ia__bpmx"><button>½</button><button>×2</button><button>Re-detect</button></div>

    <div class="ia__row">
      <Stars value={4} size={15} />
      <Swatches selected={4} size={13} />
    </div>

    <div class="ia__grid">
      <span class="ia__lab">Genre</span>
      <div class="ia__val"><Fld value={genreEdit ? 'Melodic Te' : t.genre} state={genreEdit ? 'edit' : 'rest'} suggest={genreEdit ? ['Melodic Techno', 'Melodic House & Techno'] : null} /></div>
      <span class="ia__lab">Label</span>
      <div class="ia__val"><Fld value={t.label} /></div>
      <span class="ia__lab">Year</span>
      <div class="ia__val"><Fld value={String(t.year)} mono /></div>
      <span class="ia__lab">Added</span>
      <div class="ia__val ia__ro">2 days ago</div>
    </div>

    <div class="ia__sec"><span>COMMENT</span></div>
    <Fld value={t.comment} multiline height={52} placeholder="Add a note" />

    {#if cueRows}
      <div class="ia__sec"><span>HOT CUES</span><span class="ia__n">5</span><button class="ia__add"><Ico name="plus" size={12} /></button></div>
      <div class="ia__cues">
        {#each CUES as c}
          <div class="ia__cue">
            <span class="ia__cdot" style="background:{c.color}">{c.id}</span>
            <div class="ia__cname"><Fld value={c.name} /></div>
            <span class="ia__ctime">{c.position}</span>
            <button class="ia__cgo" aria-label="Jump"><Ico name="play" size={10} /></button>
          </div>
        {/each}
      </div>
    {/if}

    <div class="ia__file"><span class="ia__lab">FILE</span><span class="ia__fmeta">FLAC · 44.1 kHz · 52.3 MB</span><Ico name="chevron-down" size={12} /></div>
  </div>
</div>

<style>
  .ia { display: flex; flex-direction: column; height: 100%; background: var(--ff-surface); border-left: 1px solid var(--ff-border); font-family: var(--ff-font); color: var(--ff-text); flex: none; overflow: hidden; }
  .ia__head { flex: none; height: 40px; display: flex; align-items: center; gap: 8px; padding: 0 12px 0 14px; border-bottom: 1px solid var(--ff-border); }
  .ia__kicker { font-family: var(--ff-font-mono); font-size: var(--ff-type-label-sm); letter-spacing: 0.06em; color: var(--ff-faint); flex: 1; }
  .ia__saved { display: inline-flex; align-items: center; gap: 4px; font-size: 11px; color: var(--ff-muted); }
  .ia__hr { font-size: 11px; color: var(--ff-muted); }
  .ia__ico { border: none; background: none; color: var(--ff-muted); padding: 4px; border-radius: 3px; display: flex; }
  .ia__body { flex: 1 1 0; min-height: 0; overflow: hidden; padding: 12px 12px 10px; display: flex; flex-direction: column; gap: 10px; }
  .ia__id { display: flex; gap: 10px; }
  .ia__cover { position: relative; flex: none; width: 88px; height: 88px; border-radius: var(--ff-radius-md); background-size: cover; border: 1px solid var(--ff-border-hi); }
  .ia__swap { position: absolute; inset: auto 0 0 0; text-align: center; font-size: 10px; padding: 3px 0; background: rgba(0, 0, 0, 0.55); color: var(--ff-text-mid); opacity: 0; }
  .ia__cover:hover .ia__swap { opacity: 1; }
  .ia__idf { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; margin-top: -2px; }
  .ia__scrubs { display: flex; gap: 8px; margin-top: 14px; }
  .ia__bpmx { display: flex; gap: 6px; margin-top: -2px; }
  .ia__bpmx button { height: 20px; padding: 0 8px; font-size: 10.5px; font-family: var(--ff-font-mono); border: 1px solid var(--ff-border); background: transparent; color: var(--ff-text-mid); border-radius: 3px; }
  .ia__row { display: flex; align-items: center; justify-content: space-between; padding: 2px 8px 2px 6px; }
  .ia__grid { display: grid; grid-template-columns: 52px 1fr; row-gap: 2px; align-items: center; }
  .ia__lab { font-family: var(--ff-font-mono); font-size: var(--ff-type-label-sm); letter-spacing: 0.06em; color: var(--ff-faint); text-transform: uppercase; padding-left: 2px; }
  .ia__val { min-width: 0; }
  .ia__ro { font-size: 12.5px; color: var(--ff-muted); padding-left: 9px; }
  .ia__sec { display: flex; align-items: center; gap: 6px; font-family: var(--ff-font-mono); font-size: var(--ff-type-label-sm); letter-spacing: 0.06em; color: var(--ff-faint); margin-top: 4px; }
  .ia__n { color: var(--ff-muted); flex: 1; }
  .ia__add { border: none; background: none; color: var(--ff-muted); display: flex; padding: 2px; }
  .ia__cues { display: flex; flex-direction: column; }
  .ia__cue { display: flex; align-items: center; gap: 6px; height: 28px; }
  .ia__cdot { flex: none; width: 18px; height: 18px; border-radius: 3px; display: flex; align-items: center; justify-content: center; font-family: var(--ff-font-mono); font-size: 10px; font-weight: 600; color: #0a0a0a; }
  .ia__cname { flex: 1; min-width: 0; }
  .ia__ctime { font-family: var(--ff-font-mono); font-size: 11px; color: var(--ff-text-mid); width: 36px; text-align: right; }
  .ia__cgo { border: none; background: none; color: var(--ff-muted); display: flex; padding: 4px; }
  .ia__file { display: flex; align-items: center; gap: 8px; margin-top: auto; padding-top: 8px; border-top: 1px solid var(--ff-border); color: var(--ff-muted); }
  .ia__fmeta { flex: 1; font-family: var(--ff-font-mono); font-size: 11px; }
</style>
