<script>
  import { Waveform } from '../../../../viewport/design-system/svelte.js';
  import Ico from './Ico.svelte';
  import Fld from './Fld.svelte';
  import Scrub from './Scrub.svelte';
  import { SELECTED, CUES, DETAIL_BANDS, CUE_COLORS } from './data.js';

  let { width = 340, showHeader = true } = $props();
  const t = SELECTED;
  const TABS = ['Info', 'Cues', 'Grid', 'File'];
</script>

<div class="ib" style="width:{width}px">
  {#if showHeader}
    <div class="ib__head">
      <span class="ib__kicker">TRACK</span>
      <span class="ib__saved"><Ico name="check" size={11} /> Saved</span>
      <button class="ib__ico" aria-label="Track actions"><Ico name="ellipsis" size={14} /></button>
    </div>
  {/if}
  <div class="ib__id">
    <div class="ib__cover" style="background-image:url({t.cover})"></div>
    <div class="ib__idf">
      <Fld value={t.title} big />
      <Fld value={t.artist} />
    </div>
  </div>
  <div class="ib__scrubs">
    <Scrub label="BPM" value="125.00" big={false} extras={false} />
    <Scrub label="KEY" value="8A" color="var(--ff-key-8)" big={false} extras={false} />
    <div class="ib__rate"><span>RATING</span><b>★★★★☆</b></div>
  </div>
  <div class="ib__tabs">
    {#each TABS as tab, i}
      <button class="ib__tab" class:ib__tab--on={i === 1}>{tab}{#if i === 1}<span class="ib__tn">5</span>{/if}</button>
    {/each}
  </div>

  <div class="ib__body">
    <div class="ib__wave">
      <Waveform bands={DETAIL_BANDS} width={316} height={46} variant="strip" look="bands" />
      {#each CUES as c, i}
        <span class="ib__mark" class:ib__mark--drag={i === 2} style="left:{c.frac * 100}%;background:{c.color}"><em style="background:{c.color}">{c.id}</em></span>
      {/each}
      <span class="ib__head-pos" style="left:45%"></span>
      {#if true}<span class="ib__tip" style="left:47%">3:52 · beat 125</span>{/if}
    </div>

    <div class="ib__cues">
      {#each CUES as c, i}
        <div class="ib__cue" class:ib__cue--sel={i === 2}>
          <button class="ib__cdot" style="background:{c.color}">{c.id}</button>
          <div class="ib__cname"><Fld value={c.name} state={i === 2 ? 'edit' : 'rest'} /></div>
          <div class="ib__ctime">
            <button aria-label="Earlier by one beat"><Ico name="chevron-left" size={11} /></button>
            <span>{c.position}</span>
            <button aria-label="Later by one beat"><Ico name="chevron-right" size={11} /></button>
          </div>
          <button class="ib__cx" aria-label="Delete cue"><Ico name="x" size={12} /></button>
          {#if i === 2}
            <div class="ib__pop">
              <div class="ib__pop-title">Colour</div>
              <div class="ib__pop-sw">
                {#each CUE_COLORS as col, k}
                  <span style="background:{col}" class:on={k === 2}></span>
                {/each}
              </div>
              <div class="ib__pop-foot"><span>Set to playhead</span><kbd>⌥ click</kbd></div>
            </div>
          {/if}
        </div>
      {/each}
    </div>
    <button class="ib__add"><Ico name="plus" size={12} /> Add cue at playhead <span>3:41</span></button>
    <div class="ib__note">Drag a marker to move it · snaps to beats · ⌥ for free</div>
  </div>
</div>

<style>
  .ib { display: flex; flex-direction: column; height: 100%; background: var(--ff-surface); border-left: 1px solid var(--ff-border); font-family: var(--ff-font); color: var(--ff-text); flex: none; overflow: hidden; }
  .ib__head { flex: none; height: 40px; display: flex; align-items: center; gap: 8px; padding: 0 12px 0 14px; border-bottom: 1px solid var(--ff-border); }
  .ib__kicker { font-family: var(--ff-font-mono); font-size: var(--ff-type-label-sm); letter-spacing: 0.06em; color: var(--ff-faint); flex: 1; }
  .ib__saved { display: inline-flex; align-items: center; gap: 4px; font-size: 11px; color: var(--ff-muted); }
  .ib__ico { border: none; background: none; color: var(--ff-muted); padding: 4px; display: flex; }
  .ib__id { display: flex; gap: 10px; padding: 12px 12px 4px; }
  .ib__cover { flex: none; width: 52px; height: 52px; border-radius: var(--ff-radius-md); background-size: cover; border: 1px solid var(--ff-border-hi); }
  .ib__idf { flex: 1; min-width: 0; display: flex; flex-direction: column; justify-content: center; }
  .ib__scrubs { display: flex; gap: 8px; padding: 6px 12px 12px; align-items: flex-end; }
  .ib__rate { flex: 1; display: flex; flex-direction: column; gap: 3px; font-family: var(--ff-font-mono); font-size: var(--ff-type-label-sm); letter-spacing: 0.06em; color: var(--ff-faint); }
  .ib__rate b { font-weight: 400; font-size: 13px; letter-spacing: 1px; color: var(--ff-text); height: 32px; display: flex; align-items: center; }
  .ib__tabs { flex: none; display: flex; gap: 2px; padding: 0 10px; border-bottom: 1px solid var(--ff-border); }
  .ib__tab { border: none; background: none; padding: 8px 10px; font-family: var(--ff-font); font-size: 12.5px; color: var(--ff-muted); border-bottom: 1.5px solid transparent; margin-bottom: -1px; display: flex; gap: 6px; }
  .ib__tab--on { color: var(--ff-text); border-bottom-color: var(--ff-accent); }
  .ib__tn { font-family: var(--ff-font-mono); font-size: 10.5px; color: var(--ff-muted); }
  .ib__body { flex: 1 1 0; min-height: 0; padding: 12px; display: flex; flex-direction: column; gap: 10px; overflow: hidden; }
  .ib__wave { position: relative; height: 46px; border-radius: var(--ff-radius-sm); overflow: visible; }
  .ib__mark { position: absolute; top: -2px; bottom: -2px; width: 2px; margin-left: -1px; border-radius: 1px; }
  .ib__mark em { position: absolute; top: -9px; left: -6px; width: 14px; height: 12px; border-radius: 2px; font-style: normal; font-family: var(--ff-font-mono); font-size: 8.5px; font-weight: 700; color: #0a0a0a; display: flex; align-items: center; justify-content: center; }
  .ib__mark--drag { width: 3px; box-shadow: 0 0 0 3px rgba(255, 255, 255, 0.15); }
  .ib__head-pos { position: absolute; top: 0; bottom: 0; width: 1px; background: #fff; opacity: 0.8; }
  .ib__tip { position: absolute; top: 52px; transform: translateX(-50%); font-family: var(--ff-font-mono); font-size: 10px; color: #0a0a0a; background: var(--ff-accent); padding: 2px 6px; border-radius: 3px; white-space: nowrap; z-index: 3; }
  .ib__cues { display: flex; flex-direction: column; margin-top: 14px; }
  .ib__cue { position: relative; display: flex; align-items: center; gap: 8px; height: 34px; padding: 0 4px; border-radius: var(--ff-radius-md); }
  .ib__cue--sel { background: var(--ff-select); }
  .ib__cdot { flex: none; width: 22px; height: 22px; border: none; border-radius: 4px; font-family: var(--ff-font-mono); font-size: 11px; font-weight: 700; color: #0a0a0a; }
  .ib__cname { flex: 1; min-width: 0; }
  .ib__ctime { display: flex; align-items: center; gap: 2px; font-family: var(--ff-font-mono); font-size: 11.5px; color: var(--ff-text-mid); }
  .ib__ctime span { width: 38px; text-align: center; }
  .ib__ctime button, .ib__cx { border: none; background: none; color: var(--ff-faint); display: flex; padding: 4px; border-radius: 3px; }
  .ib__cue:hover .ib__ctime button, .ib__cue--sel .ib__ctime button { color: var(--ff-text-mid); }
  .ib__pop { position: absolute; z-index: 10; left: 0; top: 38px; width: 196px; background: var(--ff-elev); border: 1px solid var(--ff-border-hi); border-radius: var(--ff-radius-lg); box-shadow: var(--ff-shadow-menu); padding: 8px 10px; display: flex; flex-direction: column; gap: 8px; }
  .ib__pop-title { font-family: var(--ff-font-mono); font-size: var(--ff-type-label-sm); letter-spacing: 0.06em; color: var(--ff-faint); text-transform: uppercase; }
  .ib__pop-sw { display: flex; gap: 6px; }
  .ib__pop-sw span { width: 18px; height: 18px; border-radius: 3px; }
  .ib__pop-sw span.on { outline: 1.5px solid var(--ff-accent); outline-offset: 2px; }
  .ib__pop-foot { display: flex; justify-content: space-between; align-items: center; font-size: 11.5px; color: var(--ff-text-mid); border-top: 1px solid var(--ff-border); padding-top: 7px; }
  .ib__pop-foot kbd { font-family: var(--ff-font-mono); font-size: 10px; color: var(--ff-muted); }
  .ib__add { display: flex; align-items: center; gap: 6px; height: 30px; padding: 0 10px; margin-top: 4px; border: 1px dashed var(--ff-border-hi); background: transparent; color: var(--ff-text-mid); font-size: 12px; border-radius: var(--ff-radius-md); font-family: var(--ff-font); }
  .ib__add span { margin-left: auto; font-family: var(--ff-font-mono); color: var(--ff-muted); font-size: 11px; }
  .ib__note { font-size: 11px; color: var(--ff-faint); }
</style>
