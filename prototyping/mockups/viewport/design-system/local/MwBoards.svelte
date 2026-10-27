<script>
  import { FilterInput, Kbd, SegmentedControl, StatusDot, Spinner, Icon } from '../../../../viewport/design-system/svelte.js';
  import Win from './Win.svelte';
  import Frame from './Frame.svelte';
  import Ico from './Ico.svelte';
  import NowRight from './NowRight.svelte';
  import BoardList from './BoardList.svelte';
  import InspA from './InspA.svelte';
  import InspC from './InspC.svelte';
  import Fld from './Fld.svelte';
  import Stars from './Stars.svelte';
  import Swatches from './Swatches.svelte';
  import Scrub from './Scrub.svelte';
  import { SELECTED, CUES, TRACKS } from './data.js';

  let { variant = 'now', boardOpen = true, bottom = 'above-player' } = $props();
  const t = SELECTED;
  const META = {
    now: {
      tag: 'NOW',
      title: 'Main window today (inspector open)',
      model: 'Search-only top bar. Inspector and organize drawer both live on the right, behind a cover rail.',
      good: ['Browse, inspect and put are all reachable without modes', 'Rail with covers is a nice glance at the board'],
      cost: [
        'Top bar holds one search field and a ⌘F hint: 42px of chrome that does nothing else',
        'Cover rail eats 46px for a glance; its chevron looks like a different control than the drawer header icons',
        'The inspector shows a big cover and a waveform that the player already shows; KEY and TIME are tight',
      ],
    },
    'now-open': {
      tag: 'NOW',
      title: 'Main window today (organize drawer open)',
      model: 'Drawer replaces the inspector. Header carries a "panel" icon and an "x" that look like the same thing.',
      good: ['The board stays next to the library: drag left to right works'],
      cost: [
        '"Panel" icon (collapse to rail) next to "x" (close and clear): same weight, no labels, nobody knows the difference',
        'The inspector disappears with no way to peek at a track while organizing',
      ],
    },
    a: {
      tag: 'A',
      title: 'One right dock, two tabs',
      model: 'Track and Board are two tabs of one right dock. One toggle in the top bar shows or hides the whole dock. No x, no panel icon in headers.',
      good: [
        'One right-hand thing to learn; the cover rail is gone (46px back)',
        'F16 solved by removal: the only close is the top bar toggle (⇧⇥), tab switch is not closing',
        'Dragging a track out auto-switches to the Board tab, so the drop target is always there',
      ],
      cost: [
        'Track and Board are never visible together: no peeking at BPM/key while placing',
        'The top bar now has status chips: needs discipline to stay small',
      ],
    },
    b: {
      tag: 'B',
      title: 'Track strip below, board on the right',
      model: 'Three fixed places: left browses, right puts, bottom shows the track you are on. Top bar is a filter bar for a 30k library.',
      good: [
        'Track details and board are visible together; inspecting never evicts the board',
        'Inspector gets the width it needs: every field plus cues on the waveform, no scrolling',
        'F16: board header has two labelled verbs: Widen (⤢) and Hide (›). Clearing is in the target menu',
      ],
      cost: [
        'The strip costs about 330px of table height (collapsible with a chevron, ⌘I)',
        'Bigger redesign of the inspector and the player boundary (cues now live in two places)',
      ],
    },
    c: {
      tag: 'C',
      title: 'Inline row editor, board on the right',
      model: 'The selected row expands in place into a compact editor. The right side is only the board. Top bar is one omnibox (⌘K), status chips on the right.',
      good: [
        'Zero extra panels: the track you edit stays in the list context, neighbours visible above and below',
        'The right side is purely "put": the original left=browse, right=put model, nothing else on the right',
        'Omnibox unifies ⌘F and ⌘K and gives the empty top bar real work',
      ],
      cost: [
        'Rows shift down when a row expands; keyboard stepping through rows needs a rule (expand follows selection or on Space/Enter)',
        'Dense: the editor is a fixed-height form, cues editing is limited to chips (waveform editing stays in the expanded player)',
      ],
    },
    d: {
      tag: 'D',
      title: 'Board is a full-height column outside the main view',
      model: 'Main view keeps search, list and inspector as today. The board becomes a column at the window edge, from the top bar level down. One toggle in the top bar next to the search (⇧⇥).',
      good: [
        'The inspector is never squeezed or covered: it stays beside the list, the board sits outside',
        'F16 solved by removal: one toggle in the top bar shows its state; the board header has no panel icon and no x, only "Change"',
        'The cover rail is gone: a closed board costs 0px, not 46px',
      ],
      cost: [
        'Open board takes 360px from the main view; at 1100px it overlays instead of squeezing',
        'Clearing the target moves from an x to a labelled "Change" button',
      ],
    },
  };
  const m = $derived(META[variant]);
</script>

{#snippet boardCol()}
  <div class="dock dock--d">
    <div class="dock__head dock__head--band">
      <span class="target__n">Peak Time</span>
      <span class="target__c">148 tracks · 14h 12m</span>
      <button class="chg">Change</button>
    </div>
    <div class="dock__body"><BoardList dropAfter={4} /></div>
  </div>
{/snippet}

<Frame tag={m.tag} title={m.title} model={m.model} good={m.good} cost={m.cost}>
  {#if variant === 'now'}
    <Win cols="mid">
      {#snippet right()}<NowRight mode="inspector" />{/snippet}
    </Win>
  {:else if variant === 'now-open'}
    <Win cols="narrow">
      {#snippet right()}<NowRight mode="drawer-filled" drawerWidth={440} />{/snippet}
    </Win>
  {:else if variant === 'd'}
    <Win cols={boardOpen ? 'narrow' : 'mid'}>
      {#snippet top()}
        <div class="tb tb--d">
          <div class="tb__search"><FilterInput placeholder="Search tracks, artists, labels…" /><Kbd key="⌘F" /></div>
          <button class="tb__dock" class:tb__dock--on={boardOpen} aria-label="Toggle board" aria-pressed={boardOpen}><Ico name="panel" size={15} /></button>
        </div>
      {/snippet}
      {#snippet right()}<NowRight mode="inspector-only" />{/snippet}
      {#snippet edge()}
        {#if boardOpen && bottom === 'above-player'}{@render boardCol()}{/if}
      {/snippet}
      {#snippet edgeFull()}
        {#if boardOpen && bottom === 'full'}{@render boardCol()}{/if}
      {/snippet}
    </Win>
  {:else if variant === 'a'}
    <Win cols="mid">
      {#snippet top()}
        <div class="tb">
          <div class="tb__ctx"><b>All Tracks</b><span>30,412</span></div>
          <div class="tb__search"><FilterInput placeholder="Filter this list…" /><Kbd key="⌘F" /></div>
          <div class="tb__right">
            <span class="chip"><Spinner size="small" /> Analyzing <b>120 / 340</b></span>
            <span class="chip"><StatusDot status="online" /> SANDISK_64 <b>Synced</b></span>
            <span class="tb__sep"></span>
            <Kbd key="⌘K" />
            <button class="tb__dock tb__dock--on" aria-label="Hide dock"><Ico name="panel" size={15} /></button>
          </div>
        </div>
      {/snippet}
      {#snippet right()}
        <div class="dock dock--a">
          <div class="dock__head">
            <div class="dock__seg"><SegmentedControl items={['Track', 'Board']} activeIndex={0} /></div>
            <span class="dock__sub">Board: Peak Time · 148</span>
          </div>
          <div class="dock__body"><InspA width={360} showHeader={false} /></div>
          <div class="callout">Dragging tracks switches this dock to Board</div>
        </div>
      {/snippet}
    </Win>
  {:else if variant === 'b'}
    <Win cols="mid" selectedId={1}>
      {#snippet top()}
        <div class="tb">
          <div class="tb__search tb__search--w"><FilterInput placeholder="Filter 30,412 tracks…" /></div>
          <span class="fchip">BPM <b>120–128</b><Ico name="x" size={11} /></span>
          <span class="fchip fchip--key"><i style="background:var(--ff-key-8)"></i>Key <b>8A ± 1</b><Ico name="x" size={11} /></span>
          <span class="fchip">Rating <b>≥ 3</b><Ico name="x" size={11} /></span>
          <span class="fchip fchip--add"><Ico name="plus" size={11} /> Filter</span>
          <span class="tb__count">2,184 of 30,412</span>
          <div class="tb__right">
            <Kbd key="⌘K" />
            <button class="tb__dock tb__dock--on" aria-label="Hide board"><Ico name="panel" size={15} /></button>
          </div>
        </div>
      {/snippet}
      {#snippet below()}<InspC laneWidth={800} keyOpen={false} />{/snippet}
      {#snippet right()}
        <div class="dock dock--b">
          <div class="dock__head">
            <button class="target"><span class="target__n">Peak Time</span><span class="target__c">148 tracks · 14h 12m</span><Ico name="chevron-down" size={12} /></button>
            <button class="dock__ib" aria-label="Widen board"><Ico name="maximize" size={14} /></button>
            <button class="dock__ib" aria-label="Hide board"><Ico name="chevron-right" size={15} /></button>
          </div>
          <div class="dock__body"><BoardList dropAfter={4} /></div>
        </div>
      {/snippet}
    </Win>
  {:else if variant === 'c'}
    <Win cols="mid" expandedId={1}>
      {#snippet top()}
        <div class="tb">
          <div class="omni">
            <Ico name="search" size={14} />
            <span>Search tracks, playlists, commands…</span>
            <Kbd key="⌘K" />
          </div>
          <div class="tb__right">
            <span class="chip"><Spinner size="small" /> <b>120 / 340</b></span>
            <span class="chip"><StatusDot status="online" /> <b>Synced</b></span>
            <button class="tb__dock tb__dock--on" aria-label="Hide board"><Ico name="panel" size={15} /></button>
          </div>
        </div>
      {/snippet}
      {#snippet rowExtra()}
        <div class="rx">
          <div class="rx__cover" style="background-image:url({t.cover})"></div>
          <div class="rx__grid">
            <label>Title</label><Fld value={t.title} />
            <label>Artist</label><Fld value={t.artist} state="hover" />
            <label>Album</label><Fld value={t.album} />
            <label>Label</label><Fld value={t.label} />
            <label>Genre</label><Fld value={t.genre} />
            <label>Year</label><Fld value={String(t.year)} mono />
          </div>
          <div class="rx__num">
            <div class="rx__scr"><Scrub label="BPM" value="125.00" big={false} extras={false} /><Scrub label="KEY" value="8A" color="var(--ff-key-8)" big={false} extras={false} /></div>
            <div class="rx__rc"><Stars value={4} size={14} /><Swatches selected={4} size={12} /></div>
          </div>
          <div class="rx__com">
            <label>Comment</label>
            <Fld value={t.comment} multiline height={44} />
            <div class="rx__cues">
              {#each CUES as c}<span style="border-color:{c.color}"><i style="background:{c.color}">{c.id}</i>{c.name} {c.position}</span>{/each}
              <span class="rx__addc">+</span>
            </div>
          </div>
        </div>
      {/snippet}
      {#snippet right()}
        <div class="dock dock--b">
          <div class="dock__head">
            <button class="target"><span class="target__n">Peak Time</span><span class="target__c">148 tracks · 14h 12m</span><Ico name="chevron-down" size={12} /></button>
            <button class="dock__ib" aria-label="Widen board"><Ico name="maximize" size={14} /></button>
            <button class="dock__ib" aria-label="Hide board"><Ico name="chevron-right" size={15} /></button>
          </div>
          <div class="dock__body"><BoardList dropAfter={4} /></div>
        </div>
      {/snippet}
    </Win>
  {/if}
</Frame>

<style>
  .tb--d { justify-content: center; }
  .tb--d .tb__dock { position: absolute; right: 14px; }
  .dock--d { width: 360px; border-left: 1px solid var(--ff-border-hi); }
  .dock__head--band { height: var(--ff-titlebar-band-height); }
  .chg { border: 1px solid var(--ff-border-hi); background: none; color: var(--ff-text-mid); font-family: var(--ff-font); font-size: 11.5px; height: 24px; padding: 0 10px; border-radius: var(--ff-radius-md); }
  .tb { flex: none; height: var(--ff-titlebar-band-height); display: flex; align-items: center; gap: 12px; padding: 0 14px 0 16px; border-bottom: 1px solid var(--ff-border); background: var(--ff-bg); }
  .tb__ctx { display: flex; align-items: baseline; gap: 8px; width: 170px; flex: none; }
  .tb__ctx b { font-size: 14px; font-weight: 500; }
  .tb__ctx span { font-family: var(--ff-font-mono); font-size: 11px; color: var(--ff-muted); }
  .tb__search { flex: 1 1 0; max-width: 420px; display: flex; align-items: center; gap: 8px; }
  .tb__search--w { flex: none; width: 300px; }
  .tb__count { font-family: var(--ff-font-mono); font-size: 11px; color: var(--ff-muted); margin-left: 4px; white-space: nowrap; }
  .tb__right { margin-left: auto; display: flex; align-items: center; gap: 10px; }
  .tb__sep { width: 1px; height: 16px; background: var(--ff-border-hi); }
  .tb__dock { width: 28px; height: 26px; border: 1px solid transparent; background: none; color: var(--ff-text-mid); border-radius: var(--ff-radius-md); display: flex; align-items: center; justify-content: center; }
  .tb__dock--on { background: var(--ff-select); color: var(--ff-text); border-color: var(--ff-border); }
  .chip { display: inline-flex; align-items: center; gap: 6px; height: 24px; padding: 0 9px; border: 1px solid var(--ff-border); border-radius: var(--ff-radius-md); font-size: 11.5px; color: var(--ff-text-mid); white-space: nowrap; }
  .chip b { font-weight: 500; color: var(--ff-text); font-family: var(--ff-font-mono); font-size: 11px; }
  .fchip { display: inline-flex; align-items: center; gap: 6px; height: 24px; padding: 0 8px; border: 1px solid var(--ff-border-hi); border-radius: var(--ff-radius-md); font-size: 11.5px; color: var(--ff-text-mid); background: var(--ff-elev); white-space: nowrap; }
  .fchip b { font-weight: 500; color: var(--ff-text); font-family: var(--ff-font-mono); font-size: 11px; }
  .fchip i { width: 8px; height: 8px; border-radius: 2px; }
  .fchip--add { background: none; border-style: dashed; color: var(--ff-muted); }
  .omni { position: absolute; left: 50%; transform: translateX(-50%); display: flex; align-items: center; gap: 10px; width: 520px; height: 30px; padding: 0 10px; border: 1px solid var(--ff-border-hi); border-radius: var(--ff-radius-lg); background: var(--ff-surface); color: var(--ff-faint); font-size: 12.5px; margin-left: -40px; }
  .omni span { flex: 1; }
  .tb { position: relative; }

  .dock { flex: none; display: flex; flex-direction: column; background: var(--ff-surface); border-left: 1px solid var(--ff-border); font-family: var(--ff-font); position: relative; overflow: hidden; }
  .dock--a { width: 360px; }
  .dock--b { width: 340px; }
  .dock__head { flex: none; height: 40px; display: flex; align-items: center; gap: 6px; padding: 0 8px 0 12px; border-bottom: 1px solid var(--ff-border); }
  .dock__seg { flex: none; }
  .dock__sub { margin-left: auto; font-size: 11px; color: var(--ff-muted); }
  .dock__body { flex: 1 1 0; min-height: 0; overflow: hidden; display: flex; flex-direction: column; }
  .dock__body :global(.ia) { border-left: none; }
  .dock__ib { border: none; background: none; color: var(--ff-text-mid); display: flex; padding: 6px; border-radius: 3px; }
  .target { flex: 1; min-width: 0; display: flex; align-items: baseline; gap: 8px; border: none; background: none; padding: 4px 6px; color: var(--ff-text); border-radius: 3px; text-align: left; font-family: var(--ff-font); }
  .target__n { font-size: 13px; font-weight: 500; }
  .target__c { font-size: 11px; color: var(--ff-muted); flex: 1; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .callout { position: absolute; left: 12px; right: 12px; bottom: 10px; padding: 6px 10px; font-size: 11px; color: var(--ff-text-mid); border: 1px dashed var(--ff-border-hi); border-radius: var(--ff-radius-md); background: var(--ff-elev); text-align: center; }

  .rx { display: grid; grid-template-columns: 112px 1.3fr 0.9fr 1.1fr; gap: 14px; padding: 10px 12px 12px 14px; background: var(--ff-surface); border-top: 1px solid var(--ff-border); border-bottom: 1px solid var(--ff-border-hi); box-shadow: inset 2px 0 0 var(--ff-accent); }
  .rx__cover { width: 112px; height: 112px; border-radius: var(--ff-radius-md); background-size: cover; border: 1px solid var(--ff-border-hi); }
  .rx__grid { display: grid; grid-template-columns: 46px 1fr; align-content: start; row-gap: 1px; align-items: center; }
  .rx label { font-family: var(--ff-font-mono); font-size: var(--ff-type-label-sm); letter-spacing: 0.06em; color: var(--ff-faint); text-transform: uppercase; }
  .rx__num { display: flex; flex-direction: column; gap: 10px; }
  .rx__scr { display: flex; gap: 8px; }
  .rx__rc { display: flex; align-items: center; justify-content: space-between; padding: 0 4px; }
  .rx__com { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .rx__cues { display: flex; flex-wrap: wrap; gap: 4px; }
  .rx__cues span { display: inline-flex; align-items: center; gap: 4px; height: 20px; padding: 0 6px 0 2px; border: 1px solid; border-radius: 3px; font-size: 10.5px; color: var(--ff-text-mid); font-family: var(--ff-font-mono); }
  .rx__cues i { width: 14px; height: 14px; border-radius: 2px; font-style: normal; font-size: 9px; font-weight: 700; color: #0a0a0a; display: flex; align-items: center; justify-content: center; }
  .rx__cues .rx__addc { border-style: dashed; border-color: var(--ff-border-hi); padding: 0 8px; color: var(--ff-muted); }
</style>
