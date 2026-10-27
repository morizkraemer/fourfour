<script>
  import { Icon, StatusDot, ProgressBar } from '../../../../viewport/design-system/svelte.js';
  import Win from './Win.svelte';
  import Frame from './Frame.svelte';
  import Ico from './Ico.svelte';
  import NowRight from './NowRight.svelte';
  import BoardList from './BoardList.svelte';
  import { TRACKS, PLAYLIST_ROWS, cover } from './data.js';

  let { variant = 'now' } = $props();

  const META = {
    now: {
      tag: 'NOW',
      title: 'Organize drawer today: empty state (left) and with a playlist (right)',
      model: 'Header with a panel icon and an x. Empty body is a blank column; New playlist, search and a short list sit at the bottom.',
      good: ['Search plus recents is the right content for the picker', 'Drag from the library to the filled board works'],
      cost: [
        '700px of nothing above "New playlist": the empty state is a void and the actions float mid-panel',
        'Panel icon and x: one hides the drawer, the other clears the target; nothing says which',
        'USB devices are an afterthought (one row) though sticks are the point of the product',
      ],
    },
    a: {
      tag: 'A',
      title: 'Destination cards',
      model: 'Empty state is a top-anchored picker of cards: recent playlists, a dashed New playlist card, and sticks with capacity. Click opens, dropping a track on a card puts it there.',
      good: [
        'Top-anchored, nothing floats: the first thing you see is where you can put tracks',
        'Sticks get first-class cards with capacity and Synced; the stick design from earlier rounds is kept',
        'F16: two labelled verbs: Widen (⤢) and Hide (›). "Clear" is in the target menu; no x',
      ],
      cost: [
        'Cards take more room than a list; with 30 playlists it needs search first',
        'Two ways to start a playlist (card or ⌘⇧N); fine, both create it named',
      ],
    },
    b: {
      tag: 'B',
      title: 'Drop to start',
      model: 'The drawer\'s header is the picker (combobox). The empty body is one big drop zone: drop tracks and a playlist is created and named for you. No New playlist button needed.',
      good: [
        'The shortest path for the main job: drag tracks right, done; creation is a side effect of the drop',
        'Header combobox opens recents, search and sticks: one control for "where am I putting things"',
        'Smallest change: same drawer, same list, just a better empty state',
      ],
      cost: [
        'Implicit creation can litter the library with "Set 26 Oct" playlists (needs an easy undo or auto-clean of empties)',
        'Less discoverable for sticks than cards (they live in the combobox)',
      ],
    },
    c: {
      tag: 'C',
      title: 'Set builder',
      model: 'The board is an ordered set, not a list. Empty state offers "start with the selected track" and key/BPM-compatible next tracks. Filled state shows transitions.',
      good: [
        'Turns the board into what a DJ prepares: an order with smooth transitions (BPM delta, key relation) at a glance',
        'Empty state is useful: compatible suggestions from the track you selected',
        'Same two verbs in the header; the board is still just "right = put"',
      ],
      cost: [
        'Strongest opinion: assumes ordering matters (it does not for a stick or a crate)',
        'Needs good key/BPM suggestions (depends on analysis quality); more to build',
      ],
    },
  };
  const m = $derived(META[variant]);

  const cardCovers = (n) => [0, 1, 2, 3].map((i) => TRACKS[(n + i * 3) % TRACKS.length].rowCover);
  const RECENT = [
    { name: 'Peak Time', count: 148, n: 0, synced: true },
    { name: 'Warm Up', count: 96, n: 2, synced: false },
    { name: 'Saturday Closing', count: 41, n: 4, synced: true },
    { name: 'Rooftop 2026', count: 33, n: 5, synced: false },
  ];
  const SUGGEST = [
    { t: TRACKS[2], d: '+1', rel: '9A', hint: 'next key up', col: 'var(--ff-key-9)' },
    { t: TRACKS[1], d: '−1', rel: '8A', hint: 'same key', col: 'var(--ff-key-8)' },
    { t: TRACKS[16], d: '−1', rel: '8B', hint: 'relative major', col: 'var(--ff-key-8)' },
    { t: TRACKS[17], d: '0', rel: '9A', hint: 'next key up', col: 'var(--ff-key-9)' },
    { t: TRACKS[6], d: '+2', rel: '11A', hint: 'two up', col: 'var(--ff-key-11)' },
  ];
  const SETLIST = [TRACKS[1], TRACKS[0], TRACKS[2], TRACKS[17], TRACKS[6]];
  const LINKS = [
    { d: '+1', k: '8A → 8A', ok: true },
    { d: '+1', k: '8A → 9A', ok: true },
    { d: '−1', k: '9A → 9A', ok: true },
    { d: '+2', k: '9A → 11A', ok: false },
  ];
</script>

{#snippet head(label, sub, verbs = true)}
  <div class="dh">
    <button class="dh__target"><span class="dh__n">{label}</span><span class="dh__c">{sub}</span><Ico name="chevron-down" size={12} /></button>
    {#if verbs}
      <button class="dh__ib" title="Widen to full window"><Ico name="maximize" size={14} /></button>
      <button class="dh__ib" title="Hide board (⇧⇥)"><Ico name="chevron-right" size={15} /></button>
    {/if}
  </div>
{/snippet}

{#snippet mosaic(n)}
  <span class="mos">{#each cardCovers(n) as c}<i style="background-image:url('{c}')"></i>{/each}</span>
{/snippet}

{#snippet drawerA()}
  <div class="dr">
    <div class="dh"><span class="dh__title">Put tracks into</span>
      <button class="dh__ib" title="Widen to full window"><Ico name="maximize" size={14} /></button>
      <button class="dh__ib" title="Hide board (⇧⇥)"><Ico name="chevron-right" size={15} /></button>
    </div>
    <div class="dr__body">
      <div class="srch"><Ico name="search" size={13} /><span>Find playlist or stick…</span><kbd>⌘P</kbd></div>
      <div class="sec">RECENT</div>
      <div class="cards">
        <button class="card card--new"><Ico name="plus" size={20} /><span>New playlist</span><em>⌘⇧N</em></button>
        {#each RECENT as r}
          <button class="card">
            {@render mosaic(r.n)}
            <span class="card__n">{r.name}</span>
            <span class="card__c">{r.count} tracks{#if r.synced}<b>Synced</b>{/if}</span>
          </button>
        {/each}
      </div>
      <div class="sec">STICKS</div>
      <div class="sticks">
        <button class="stick">
          <Ico name="usb" size={18} />
          <div><span class="stick__n"><StatusDot status="online" /> SANDISK_64</span><span class="stick__c">41.2 of 64 GB · 3 playlists</span><div class="stick__bar"><i style="width:64%"></i></div></div>
        </button>
        <button class="stick stick--off">
          <Ico name="usb" size={18} />
          <div><span class="stick__n"><StatusDot status="offline" /> TOURSTICK</span><span class="stick__c">not plugged in · 12 playlists queued</span></div>
        </button>
      </div>
      <div class="hint">Drag tracks onto a card to put them there. Click a card to open it here.</div>
    </div>
  </div>
{/snippet}

{#snippet drawerB()}
  <div class="dr">
    {@render head('Choose a destination', 'recents, search, sticks')}
    <div class="dr__body">
      <div class="dz">
        <div class="dz__ico"><Ico name="arrowdown" size={26} /></div>
        <div class="dz__t">Drop tracks to start a playlist</div>
        <div class="dz__s">Named <b>Set 26 Oct</b> · rename anytime · undo with ⌘Z</div>
      </div>
      <div class="sec">OR OPEN</div>
      <div class="list">
        {#each RECENT.slice(0, 4) as r}
          <div class="lrow">{@render mosaic(r.n)}<span class="lrow__n">{r.name}</span>{#if r.synced}<b class="syn">Synced</b>{/if}<span class="lrow__c">{r.count}</span></div>
        {/each}
        <div class="lrow lrow--stick"><Ico name="usb" size={16} /><span class="lrow__n"><StatusDot status="online" /> SANDISK_64</span><span class="lrow__c">41.2 / 64 GB</span></div>
      </div>
    </div>
  </div>
{/snippet}

{#snippet drawerC()}
  <div class="dr">
    {@render head('New set', 'empty · 0 tracks')}
    <div class="dr__body">
      <div class="seed">
        <div class="seed__cov" style="background-image:url('{TRACKS[0].rowCover}')"></div>
        <div class="seed__t"><span>Selected</span><b>{TRACKS[0].title}</b><em>{TRACKS[0].artist} · 125 · <i style="color:var(--ff-key-8)">8A</i></em></div>
        <button class="seed__b">Start with this</button>
      </div>
      <div class="sec">NEXT UP · mixes with 8A · 125 BPM</div>
      <div class="sug">
        {#each SUGGEST as s}
          <div class="sug__r">
            <span class="sug__cov" style="background-image:url('{s.t.rowCover}')"></span>
            <span class="sug__t"><b>{s.t.title}</b><em>{s.t.artist}</em></span>
            <span class="sug__b">{s.t.bpm.slice(0, 3)} <i>{s.d}</i></span>
            <span class="sug__k" style="color:{s.col}">{s.rel}</span>
            <button class="sug__add"><Ico name="plus" size={13} /></button>
          </div>
        {/each}
      </div>
      <div class="hint">Or drag tracks from the library. Drop order is set order.</div>
    </div>
  </div>
{/snippet}

{#snippet filledA()}
  <div class="dr">
    {@render head('Peak Time', '148 tracks · 14h 12m')}
    <div class="dr__sub"><b class="syn">Synced</b><span>SANDISK_64 · keep updated</span></div>
    <div class="dr__list"><BoardList dropAfter={3} /></div>
    <div class="dr__foot"><span>avg 126 BPM</span><span>8A–11A</span><span>+3 selected</span></div>
  </div>
{/snippet}

{#snippet filledB()}
  <div class="dr">
    {@render head('Set 26 Oct', '3 tracks · 19m')}
    <div class="dr__list"><BoardList tracks={PLAYLIST_ROWS.slice(0, 3)} dropAfter={2} /></div>
    <div class="dr__add">Drop to add</div>
  </div>
{/snippet}

{#snippet filledC()}
  <div class="dr">
    {@render head('Saturday Closing', '5 tracks · 34m · 8A→11A')}
    <div class="curve">
      <svg viewBox="0 0 400 40" preserveAspectRatio="none"><path d="M0 34 L80 28 L160 20 L240 22 L320 12 L400 6" fill="none" stroke="var(--ff-text-mid)" stroke-width="1.5" /><path d="M0 34 L80 28 L160 20 L240 22 L320 12 L400 6 V40 H0Z" fill="rgba(255,255,255,0.06)" /></svg>
      <span>energy</span>
    </div>
    <div class="set">
      {#each SETLIST as t, i}
        <div class="set__r">
          <span class="set__n">{i + 1}</span>
          <span class="sug__cov" style="background-image:url('{t.rowCover}')"></span>
          <span class="sug__t"><b>{t.title}</b><em>{t.artist}</em></span>
          <span class="sug__b">{t.bpm.slice(0, 3)}</span>
          <span class="sug__k" style="color:var(--ff-key-{parseInt(t.key)})">{t.key}</span>
        </div>
        {#if i < LINKS.length}
          <div class="set__l" class:set__l--warn={!LINKS[i].ok}><i></i><span>{LINKS[i].d} BPM · {LINKS[i].k}{#if !LINKS[i].ok} · rough{/if}</span></div>
        {/if}
      {/each}
      <div class="set__drop"><span></span></div>
    </div>
  </div>
{/snippet}

<Frame tag={m.tag} title={m.title} model={m.model} good={m.good} cost={m.cost} width={1840}>
  <div class="pair">
    <Win cols="narrow" selectedId={1}>
      {#snippet right()}
        {#if variant === 'now'}
          <NowRight mode="drawer-empty" drawerWidth={440} />
        {:else}
          <div class="side">
            {#if variant === 'a'}{@render drawerA()}{:else if variant === 'b'}{@render drawerB()}{:else}{@render drawerC()}{/if}
          </div>
        {/if}
      {/snippet}
    </Win>
    <div class="filled">
      <div class="filled__lab">With a target open</div>
      <div class="filled__box">
        {#if variant === 'now'}
          <NowRight mode="drawer-filled" drawerWidth={440} />
        {:else if variant === 'a'}
          <div class="side">{@render filledA()}</div>
        {:else if variant === 'b'}
          <div class="side">{@render filledB()}</div>
        {:else}
          <div class="side">{@render filledC()}</div>
        {/if}
      </div>
    </div>
  </div>
</Frame>

<style>
  .pair { display: flex; gap: 24px; align-items: flex-start; }
  .filled { display: flex; flex-direction: column; gap: 8px; width: 376px; }
  .filled__lab { font-family: var(--ff-font-mono); font-size: 10px; letter-spacing: 0.08em; text-transform: uppercase; color: var(--ff-faint); padding-top: 2px; height: 14px; }
  .filled__box { height: 736px; width: 376px; border: 1px solid var(--ff-border-hi); border-radius: 8px; overflow: hidden; background: var(--ff-surface); }
  .filled__box :global(.nr--drawer) { width: 100% !important; }
  .side { width: 440px; height: 100%; flex: none; }
  .filled__box .side { width: 100%; }
  .dr { display: flex; flex-direction: column; height: 100%; background: var(--ff-surface); border-left: 1px solid var(--ff-border); font-family: var(--ff-font); color: var(--ff-text); overflow: hidden; }
  .filled__box .dr { border-left: none; }
  .dh { flex: none; height: 40px; display: flex; align-items: center; gap: 4px; padding: 0 8px 0 12px; border-bottom: 1px solid var(--ff-border); }
  .dh__title { flex: 1; font-size: 13px; font-weight: 500; }
  .dh__target { flex: 1; min-width: 0; display: flex; align-items: baseline; gap: 8px; border: none; background: none; padding: 4px 6px; margin-left: -6px; color: var(--ff-text); border-radius: 3px; text-align: left; font-family: var(--ff-font); }
  .dh__n { font-size: 13px; font-weight: 500; white-space: nowrap; }
  .dh__c { font-size: 11px; color: var(--ff-muted); flex: 1; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .dh__ib { border: none; background: none; color: var(--ff-text-mid); display: flex; padding: 6px; border-radius: 3px; }
  .dr__body { flex: 1; min-height: 0; padding: 12px; display: flex; flex-direction: column; gap: 10px; overflow: hidden; }
  .dr__sub { flex: none; display: flex; align-items: center; gap: 8px; padding: 6px 14px; border-bottom: 1px solid var(--ff-border); font-size: 11.5px; color: var(--ff-muted); }
  .dr__list { flex: 1; min-height: 0; overflow: hidden; }
  .dr__foot { flex: none; display: flex; gap: 14px; padding: 8px 14px; border-top: 1px solid var(--ff-border); font-family: var(--ff-font-mono); font-size: 11px; color: var(--ff-muted); }
  .dr__add { margin: 8px 12px 12px; height: 34px; border: 1px dashed var(--ff-border-hi); border-radius: 4px; display: flex; align-items: center; justify-content: center; font-size: 12px; color: var(--ff-muted); }
  .sec { font-family: var(--ff-font-mono); font-size: var(--ff-type-label-sm); letter-spacing: 0.06em; color: var(--ff-faint); margin-top: 6px; }
  .srch { display: flex; align-items: center; gap: 8px; height: 32px; padding: 0 10px; border: 1px solid var(--ff-border-hi); border-radius: 4px; color: var(--ff-faint); font-size: 12.5px; background: var(--ff-bg); }
  .srch span { flex: 1; }
  kbd { font-family: var(--ff-font-mono); font-size: 10px; color: var(--ff-muted); }
  .syn { font-weight: 400; font-size: 10.5px; color: var(--ff-text-mid); border: 1px solid var(--ff-border-hi); border-radius: 3px; padding: 1px 5px; }
  .hint { font-size: 11.5px; color: var(--ff-faint); margin-top: 2px; }

  .mos { display: grid; grid-template-columns: 1fr 1fr; grid-template-rows: 1fr 1fr; width: 100%; aspect-ratio: 2 / 1; gap: 1px; border-radius: 4px; overflow: hidden; }
  .mos i { background-size: cover; display: block; }
  .cards { display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 8px; }
  .card { display: flex; flex-direction: column; gap: 5px; padding: 8px; border: 1px solid var(--ff-border); border-radius: 6px; background: var(--ff-bg); color: var(--ff-text); text-align: left; font-family: var(--ff-font); }
  .card .mos { aspect-ratio: 16 / 9; }
  .card__n { font-size: 12.5px; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .card__c { display: flex; align-items: center; gap: 6px; font-size: 11px; color: var(--ff-muted); }
  .card__c b { font-weight: 400; font-size: 10px; color: var(--ff-text-mid); border: 1px solid var(--ff-border-hi); border-radius: 3px; padding: 0 4px; }
  .card--new { align-items: center; justify-content: center; border-style: dashed; border-color: var(--ff-border-hi); background: none; color: var(--ff-text-mid); min-height: 100px; gap: 6px; }
  .card--new span { font-size: 12.5px; }
  .card--new em { font-style: normal; font-family: var(--ff-font-mono); font-size: 10px; color: var(--ff-faint); }
  .sticks { display: flex; flex-direction: column; gap: 8px; }
  .stick { display: flex; align-items: center; gap: 12px; padding: 10px 12px; border: 1px solid var(--ff-border); border-radius: 6px; background: var(--ff-bg); color: var(--ff-text-mid); text-align: left; font-family: var(--ff-font); }
  .stick > div { flex: 1; display: flex; flex-direction: column; gap: 3px; }
  .stick__n { display: flex; align-items: center; gap: 6px; font-size: 12.5px; color: var(--ff-text); font-weight: 500; }
  .stick__c { font-size: 11px; color: var(--ff-muted); }
  .stick__bar { height: 3px; border-radius: 2px; background: var(--ff-elev-hi); overflow: hidden; margin-top: 2px; }
  .stick__bar i { display: block; height: 100%; background: var(--ff-text-mid); }
  .stick--off { opacity: 0.7; }

  .dz { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; height: 290px; border: 1.5px dashed rgba(255, 255, 255, 0.3); border-radius: 8px; background: rgba(255, 255, 255, 0.02); text-align: center; }
  .dz__ico { color: var(--ff-text-mid); width: 48px; height: 48px; border-radius: 50%; border: 1px solid var(--ff-border-hi); display: flex; align-items: center; justify-content: center; }
  .dz__t { font-size: 14px; font-weight: 500; }
  .dz__s { font-size: 11.5px; color: var(--ff-muted); }
  .dz__s b { color: var(--ff-text-mid); font-weight: 500; }
  .list { display: flex; flex-direction: column; }
  .lrow { display: flex; align-items: center; gap: 10px; height: 36px; padding: 0 4px; border-bottom: 1px solid var(--ff-border); }
  .lrow .mos { width: 32px; aspect-ratio: 1; flex: none; border-radius: 3px; }
  .lrow__n { flex: 1; font-size: 12.5px; display: flex; align-items: center; gap: 6px; }
  .lrow__c { font-family: var(--ff-font-mono); font-size: 11px; color: var(--ff-muted); }
  .lrow--stick { color: var(--ff-text-mid); }

  .seed { display: flex; align-items: center; gap: 10px; padding: 10px; border: 1px solid var(--ff-border-hi); border-radius: 6px; background: var(--ff-bg); }
  .seed__cov { flex: none; width: 44px; height: 44px; border-radius: 4px; background-size: cover; }
  .seed__t { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .seed__t span { font-family: var(--ff-font-mono); font-size: 9.5px; letter-spacing: 0.06em; text-transform: uppercase; color: var(--ff-faint); }
  .seed__t b { font-size: 12.5px; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .seed__t em { font-style: normal; font-size: 11px; color: var(--ff-muted); }
  .seed__t i { font-style: normal; font-family: var(--ff-font-mono); }
  .seed__b { height: 28px; padding: 0 12px; border: none; border-radius: 4px; background: var(--ff-accent); color: #0a0a0a; font-size: 12px; font-weight: 500; font-family: var(--ff-font); }
  .sug { display: flex; flex-direction: column; }
  .sug__r, .set__r { display: flex; align-items: center; gap: 10px; height: 44px; border-bottom: 1px solid var(--ff-border); padding: 0 4px; }
  .sug__cov { flex: none; width: 30px; height: 30px; border-radius: 3px; background-size: cover; border: 1px solid var(--ff-border-hi); }
  .sug__t { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .sug__t b { font-size: 12.5px; font-weight: 400; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sug__t em { font-style: normal; font-size: 11px; color: var(--ff-muted); }
  .sug__b { font-family: var(--ff-font-mono); font-size: 11.5px; color: var(--ff-text-mid); width: 56px; text-align: right; }
  .sug__b i { font-style: normal; color: var(--ff-faint); }
  .sug__k { font-family: var(--ff-font-mono); font-size: 11.5px; font-weight: 500; width: 32px; text-align: right; }
  .sug__add { border: 1px solid var(--ff-border-hi); background: none; color: var(--ff-text-mid); border-radius: 3px; width: 24px; height: 24px; display: flex; align-items: center; justify-content: center; }
  .curve { position: relative; flex: none; height: 52px; margin: 10px 12px 0; border: 1px solid var(--ff-border); border-radius: 4px; background: var(--ff-bg); }
  .curve svg { position: absolute; inset: 6px; width: calc(100% - 12px); height: calc(100% - 12px); }
  .curve span { position: absolute; left: 8px; top: 4px; font-family: var(--ff-font-mono); font-size: 9px; color: var(--ff-faint); letter-spacing: 0.06em; text-transform: uppercase; }
  .set { padding: 6px 12px; display: flex; flex-direction: column; }
  .set__r { height: 42px; border-bottom: none; }
  .set__n { width: 16px; font-family: var(--ff-font-mono); font-size: 11px; color: var(--ff-faint); text-align: center; }
  .set__l { display: flex; align-items: center; gap: 10px; height: 20px; padding-left: 5px; font-family: var(--ff-font-mono); font-size: 10.5px; color: var(--ff-status-online); }
  .set__l i { width: 1px; height: 100%; background: currentColor; margin-left: 7px; opacity: 0.6; }
  .set__l--warn { color: var(--ff-status-warn); }
  .set__drop { position: relative; height: 0; }
  .set__drop span { position: absolute; left: 0; right: 0; top: 4px; height: 2px; background: var(--ff-accent); border-radius: 1px; }
</style>
