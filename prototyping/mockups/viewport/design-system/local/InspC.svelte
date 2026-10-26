<script>
  import { Waveform } from '../../../../viewport/design-system/svelte.js';
  import Ico from './Ico.svelte';
  import Fld from './Fld.svelte';
  import Stars from './Stars.svelte';
  import Swatches from './Swatches.svelte';
  import Scrub from './Scrub.svelte';
  import KeyWheel from './KeyWheel.svelte';
  import { SELECTED, CUES, DETAIL_BANDS } from './data.js';

  let { laneWidth = 1170, keyOpen = true } = $props();
  const t = SELECTED;
</script>

<div class="ic">
  <div class="ic__head">
    <Ico name="panel-bottom" size={13} />
    <span class="ic__kicker">TRACK</span>
    <span class="ic__t">{t.title}</span>
    <span class="ic__a">{t.artist}</span>
    <span class="ic__saved"><Ico name="check" size={11} /> Saved</span>
    <button class="ic__x" aria-label="Collapse strip"><Ico name="chevron-down" size={14} /></button>
  </div>
  <div class="ic__cols">
    <div class="ic__col ic__col--id">
      <div class="ic__cover" style="background-image:url({t.cover})"></div>
      <div class="ic__f">
        <label>Title</label><Fld value={t.title} />
        <label>Artist</label><Fld value={t.artist} />
        <label>Album</label><Fld value={t.album} />
        <label>Label</label><Fld value={t.label} state="hover" />
      </div>
    </div>
    <div class="ic__col ic__col--num">
      <div class="ic__scrubs">
        <Scrub label="BPM" value="125.00" extras={false} sub="locked" />
        <div class="ic__keywrap">
          <Scrub label="KEY" value="8A" color="var(--ff-key-8)" state="hover" extras={false} sub="A min" />
          {#if keyOpen}
            <div class="ic__wheel">
              <KeyWheel selected="8A" size={168} />
              <div class="ic__wcap"><b>7A · 8A · 9A · 8B</b> mix well</div>
            </div>
          {/if}
        </div>
      </div>
      <div class="ic__rc"><Stars value={4} size={15} /><Swatches selected={4} size={13} /></div>
      <div class="ic__f ic__f--s">
        <label>Genre</label><Fld value={t.genre} />
        <label>Year</label><Fld value={String(t.year)} mono />
      </div>
    </div>
    <div class="ic__col ic__col--com">
      <label class="ic__l">COMMENT</label>
      <Fld value={t.comment} multiline height={72} placeholder="Add a note" />
      <div class="ic__file"><span>FILE</span> FLAC · 44.1 kHz · 52.3 MB · added 2 days ago</div>
      <div class="ic__file ic__file--p">/Users/merlin/Music/Kiln/Afterimage EP/01 Afterimage (Extended Mix).flac</div>
    </div>
  </div>
  <div class="ic__lane">
    <div class="ic__wave" style="width:{laneWidth}px">
      <Waveform bands={DETAIL_BANDS} width={laneWidth} height={44} variant="strip" look="bands" />
      {#each CUES as c, i}
        <span class="ic__mark" class:ic__mark--sel={i === 2} style="left:{c.frac * 100}%;background:{c.color}"><em style="background:{c.color}">{c.id}</em></span>
      {/each}
    </div>
    <div class="ic__chip">
      <span class="ic__cdot" style="background:{CUES[2].color}">C</span>
      <div class="ic__cn"><Fld value="Break" state="edit" /></div>
      <span class="ic__ct">3:52</span>
      <button>◂ beat</button><button>beat ▸</button><button>Set to playhead</button>
      <button class="ic__del"><Ico name="trash" size={12} /></button>
      <span class="ic__tip">Drag flags on the waveform · click empty waveform with ⌥ to add</span>
    </div>
  </div>
</div>

<style>
  .ic { flex: none; background: var(--ff-surface); border-top: 1px solid var(--ff-border-hi); font-family: var(--ff-font); color: var(--ff-text); position: relative; }
  .ic__head { height: 32px; display: flex; align-items: center; gap: 8px; padding: 0 12px; border-bottom: 1px solid var(--ff-border); color: var(--ff-muted); }
  .ic__kicker { font-family: var(--ff-font-mono); font-size: var(--ff-type-label-sm); letter-spacing: 0.06em; color: var(--ff-faint); }
  .ic__t { font-size: 12.5px; color: var(--ff-text); font-weight: 500; }
  .ic__a { font-size: 12px; color: var(--ff-text-mid); flex: 1; }
  .ic__saved { display: inline-flex; gap: 4px; align-items: center; font-size: 11px; }
  .ic__x { border: none; background: none; color: var(--ff-muted); display: flex; }
  .ic__cols { display: grid; grid-template-columns: 1.25fr 1fr 1fr; gap: 20px; padding: 12px 14px 8px; }
  .ic__col { min-width: 0; display: flex; flex-direction: column; gap: 8px; position: relative; }
  .ic__col--id { flex-direction: row; gap: 12px; }
  .ic__cover { flex: none; width: 104px; height: 104px; border-radius: var(--ff-radius-md); background-size: cover; border: 1px solid var(--ff-border-hi); }
  .ic__f { flex: 1; min-width: 0; display: grid; grid-template-columns: 46px 1fr; align-items: center; row-gap: 1px; }
  .ic__f--s { flex: none; grid-template-columns: 44px 1fr; }
  label, .ic__l { font-family: var(--ff-font-mono); font-size: var(--ff-type-label-sm); letter-spacing: 0.06em; color: var(--ff-faint); text-transform: uppercase; }
  .ic__scrubs { display: flex; gap: 10px; }
  .ic__keywrap { position: relative; flex: 1 1 0; display: flex; min-width: 0; }
  .ic__wheel { position: absolute; z-index: 20; left: -20px; bottom: calc(100% + 6px); width: 204px; padding: 10px 10px 8px; background: var(--ff-elev); border: 1px solid var(--ff-border-hi); border-radius: var(--ff-radius-lg); box-shadow: var(--ff-shadow-menu); display: flex; flex-direction: column; align-items: center; gap: 4px; }
  .ic__wcap { font-size: 11px; color: var(--ff-muted); }
  .ic__wcap b { font-weight: 500; font-family: var(--ff-font-mono); color: var(--ff-text-mid); }
  .ic__rc { display: flex; justify-content: space-between; align-items: center; padding: 0 6px; }
  .ic__file { font-size: 11px; color: var(--ff-muted); }
  .ic__file span { font-family: var(--ff-font-mono); font-size: var(--ff-type-label-sm); color: var(--ff-faint); letter-spacing: 0.06em; margin-right: 6px; }
  .ic__file--p { font-family: var(--ff-font-mono); font-size: 10.5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--ff-faint); }
  .ic__lane { padding: 8px 14px 12px; display: flex; flex-direction: column; gap: 8px; }
  .ic__wave { position: relative; height: 44px; margin-top: 10px; }
  .ic__mark { position: absolute; top: -4px; bottom: -2px; width: 2px; margin-left: -1px; }
  .ic__mark em { position: absolute; top: -10px; left: -7px; width: 16px; height: 13px; border-radius: 2px; font-style: normal; font-family: var(--ff-font-mono); font-size: 9px; font-weight: 700; color: #0a0a0a; display: flex; align-items: center; justify-content: center; }
  .ic__mark--sel { width: 3px; box-shadow: 0 0 0 3px rgba(255, 255, 255, 0.18); }
  .ic__chip { display: flex; align-items: center; gap: 8px; }
  .ic__cdot { width: 20px; height: 20px; border-radius: 4px; font-family: var(--ff-font-mono); font-size: 10.5px; font-weight: 700; color: #0a0a0a; display: flex; align-items: center; justify-content: center; }
  .ic__cn { width: 150px; }
  .ic__ct { font-family: var(--ff-font-mono); font-size: 12px; color: var(--ff-text-mid); }
  .ic__chip button { height: 24px; padding: 0 8px; border: 1px solid var(--ff-border); background: transparent; color: var(--ff-text-mid); font-size: 11.5px; border-radius: 3px; font-family: var(--ff-font); }
  .ic__del { padding: 0 6px !important; }
  .ic__tip { margin-left: auto; font-size: 11px; color: var(--ff-faint); }
</style>
