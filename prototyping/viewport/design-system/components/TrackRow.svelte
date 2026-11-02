<script module>
  import { columnCellClass } from '../utils/column-layout.js';

  // Default text column layout. The table module passes its own Pencil-matching
  // column set; this is the fallback so the standalone artboard stays aligned.
  export const TRACK_COLUMNS = [
    { key: 'index', label: '#', width: 26 },
    { key: 'title', label: 'TITLE', flex: true },
    { key: 'artist', label: 'ARTIST', width: 200 },
    { key: 'label', label: 'LABEL', width: 160 },
    { key: 'bpm', label: 'BPM', width: 52 },
    { key: 'time', label: 'TIME', width: 48 },
    { key: 'tag', label: '', width: 16, align: 'right' },
  ];

  const SPECIAL_KEYS = new Set(['tag', 'fav', 'wave', 'cover', 'key', 'rating', 'color']);
  const DASH_KEYS = new Set(['album', 'genre', 'year']);
  const TEXT_KINDS = new Set(['text', 'dash', 'key']);

  function buildPlan(columns) {
    return columns.map((col) => {
      const kind = SPECIAL_KEYS.has(col.key) ? col.key : DASH_KEYS.has(col.key) ? 'dash' : 'text';
      const base = `ff-trow__cell ff-trow__cell--${col.key}`;
      return { col, kind, cls: (TEXT_KINDS.has(kind) ? `${base} ff-trow__cell--text` : base) + columnCellClass(col) };
    });
  }

  // Every row of a table gets the same columns, and a sort hands them over as a new array with the same layout: one
  // plan per layout keeps each row's cells untouched unless the layout itself changed.
  const PLANS_KEPT = 8;
  const plansByColumns = new WeakMap();
  const plansByLayout = new Map();

  function layoutSignature(columns) {
    return columns.map((c) => `${c.key}|${c.flex ? 1 : 0}|${c.align}`).join(';');
  }

  function cellPlan(columns) {
    let plan = plansByColumns.get(columns);
    if (plan) return plan;
    const signature = layoutSignature(columns);
    plan = plansByLayout.get(signature);
    if (!plan) {
      plan = buildPlan(columns);
      plansByLayout.set(signature, plan);
      if (plansByLayout.size > PLANS_KEPT) plansByLayout.delete(plansByLayout.keys().next().value);
    }
    plansByColumns.set(columns, plan);
    return plan;
  }

  function favOn(v) {
    return v != null && v !== false && v !== '';
  }
</script>

<script>
  // Track list row (Svelte port of track-row.js). Cells are driven by a `columns`
  // array so a ColumnHeader built from the same array stays aligned.
  //   track: { index, title, artist, label, bpm, key, time, tag, fav, cover, bands, peaks }
  //     tag: null | { type:'color', color } | { type:'digit', value, color }
  //   state: 'rest' | 'hover' | 'selected' | 'analyzing' | 'drag'
  import './track-row.css';
  import TagBadge from './TagBadge.svelte';
  import ColorSwatch from './ColorSwatch.svelte';
  import Waveform from './Waveform.svelte';
  import { keyColor } from '../utils/key-color.js';
  import { labelColor } from '../utils/label-color.js';

  // onWaveSeek(frac): the mini waveform becomes a seek strip (hover shows where, a click passes the track fraction).
  // onRate(stars): the rating cell takes clicks (the current rating again clears it) and previews on hover.
  // holdWave: the table is flinging; the mini waveform waits to draw new data until this turns false.
  let { track = {}, state: rowState = 'rest', columns = TRACK_COLUMNS, analyzing = false, onWaveSeek = undefined, onRate = undefined, holdWave = false } = $props();

  let previewStars = $state(0);

  let plan = $derived(cellPlan(columns));
  let waveWidth = $derived(columns.find((c) => c.key === 'wave')?.width ?? 120);

  function waveRect(e) {
    return (e.currentTarget.querySelector('canvas') ?? e.currentTarget).getBoundingClientRect();
  }

  function waveFrac(e) {
    const rect = waveRect(e);
    return rect.width > 0 ? Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width)) : 0;
  }

  function showWaveCursor(e) {
    const rect = waveRect(e);
    const cell = e.currentTarget.getBoundingClientRect();
    const x = Math.max(rect.left, Math.min(rect.right, e.clientX)) - cell.left;
    e.currentTarget.style.setProperty('--ff-wave-cursor-x', `${Math.round(x)}px`);
  }

  function cover(t) {
    return t.rowCover ?? t.cover;
  }
</script>

<div class="ff-trow ff-trow--{rowState}">
  {#each plan as cell}
    {@const col = cell.col}
    {#if cell.kind === 'tag'}
      <div class={cell.cls} data-col-key={col.key}>
        {#if track.tag && track.tag.type === 'digit'}
          <TagBadge value={track.tag.value} variant="color" color={track.tag.color} size="sm" />
        {:else if track.tag}
          <ColorSwatch color={track.tag.color} />
        {/if}
      </div>
    {:else if cell.kind === 'fav'}
      <div class={cell.cls} data-col-key={col.key}>
        <div class="ff-trow__fav{favOn(track.fav) ? ' ff-trow__fav--on' : ''}">
          {#if favOn(track.fav) && track.fav !== true}
            <span class="ff-trow__fav-digit">{track.fav}</span>
          {/if}
        </div>
      </div>
    {:else if cell.kind === 'wave'}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="{cell.cls}{onWaveSeek ? ' ff-trow__cell--seekable' : ''}"
       
        data-col-key={col.key}
        data-wave-seek={onWaveSeek ? '' : undefined}
        onpointermove={onWaveSeek ? showWaveCursor : undefined}
        onclick={onWaveSeek ? (e) => onWaveSeek(waveFrac(e)) : undefined}>
        <Waveform
          bands={track.bands ?? null}
          peaks={track.peaks ?? null}
          animating={analyzing && !track.bands && !track.peaks?.length}
          width={waveWidth}
          height={14}
          variant="mini"
          hold={holdWave} />
        {#if onWaveSeek}<div class="ff-trow__wave-cursor"></div>{/if}
      </div>
    {:else if cell.kind === 'cover'}
      <div class={cell.cls} data-col-key={col.key}>
        <div
          class="ff-trow__cover"
          style={cover(track) ? `background-image:url(${cover(track)})` : undefined}>
        </div>
      </div>
    {:else if cell.kind === 'rating'}
      {@const stars = Number(track.rating) || 0}
      {@const shown = previewStars || stars}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="{cell.cls}{onRate ? ' ff-trow__cell--rateable' : ''}"
       
        data-col-key={col.key}
        data-rate={onRate ? '' : undefined}
        onpointerleave={onRate ? () => (previewStars = 0) : undefined}
        onclick={onRate ? (e) => e.stopPropagation() : undefined}
        ondblclick={onRate ? (e) => e.stopPropagation() : undefined}>
        {#each [1, 2, 3, 4, 5] as n}
          <span
            class="ff-trow__star{n <= shown ? ' ff-trow__star--on' : ''}{previewStars && n <= previewStars ? ' ff-trow__star--preview' : ''}"
            data-star={n}
            onpointerenter={onRate ? () => (previewStars = n) : undefined}
            onclick={onRate ? () => onRate(n === stars ? 0 : n) : undefined}>★</span>
        {/each}
      </div>
    {:else if cell.kind === 'color'}
      <div class={cell.cls} data-col-key={col.key}>
        {#if labelColor(track.color)}
          <ColorSwatch color={labelColor(track.color)} />
        {/if}
      </div>
    {:else if cell.kind === 'key'}
      {@const k = String(track.key ?? '—')}
      <div class={cell.cls} data-col-key={col.key}>
        <span class="ff-trow__text" style="color:{k === '—' ? 'var(--ff-muted)' : keyColor(k)}">{k}</span>
      </div>
    {:else if cell.kind === 'dash'}
      <div class={cell.cls} data-col-key={col.key}>
        <span class="ff-trow__text">{track[col.key] ?? '—'}</span>
      </div>
    {:else}
      <div class={cell.cls} data-col-key={col.key}>
        <span class="ff-trow__text">{track[col.key] ?? ''}</span>
      </div>
    {/if}
  {/each}
</div>
