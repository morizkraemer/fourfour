<script module>
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
</script>

<script>
  // Track list row (Svelte port of track-row.js). Cells are driven by a `columns`
  // array so a ColumnHeader built from the same array stays aligned.
  //   track: { index, title, artist, label, bpm, key, time, tag, fav, cover, peaks }
  //     tag: null | { type:'color', color } | { type:'digit', value, color }
  //   state: 'rest' | 'hover' | 'selected' | 'analyzing' | 'drag'
  import './track-row.css';
  import TagBadge from './TagBadge.svelte';
  import ColorSwatch from './ColorSwatch.svelte';
  import Waveform from './Waveform.svelte';
  import { keyColor } from '../utils/key-color.js';

  let { track = {}, state = 'rest', columns = TRACK_COLUMNS } = $props();

  function cellStyle(col) {
    const base = col.flex ? 'flex:1 1 0' : `width:${col.width}px;flex:none`;
    return col.align === 'right' ? `${base};justify-content:flex-end` : base;
  }

  // track.fav: favorite slot number (1..8) or true (filled, no digit) or falsy.
  function favOn(v) {
    return v != null && v !== false && v !== '';
  }
</script>

<div class="ff-trow ff-trow--{state}">
  {#each columns as col}
    {#if col.key === 'tag'}
      <div class="ff-trow__cell ff-trow__cell--tag" style={cellStyle(col)}>
        {#if track.tag && track.tag.type === 'digit'}
          <TagBadge value={track.tag.value} variant="color" color={track.tag.color} size="sm" />
        {:else if track.tag}
          <ColorSwatch color={track.tag.color} />
        {/if}
      </div>
    {:else if col.key === 'fav'}
      <div class="ff-trow__cell ff-trow__cell--fav" style={cellStyle(col)}>
        <div class="ff-trow__fav{favOn(track.fav) ? ' ff-trow__fav--on' : ''}">
          {#if favOn(track.fav) && track.fav !== true}
            <span class="ff-trow__fav-digit">{track.fav}</span>
          {/if}
        </div>
      </div>
    {:else if col.key === 'wave'}
      <div class="ff-trow__cell ff-trow__cell--wave" style={cellStyle(col)}>
        <Waveform
          peaks={track.peaks ?? null}
          width={col.width ?? 120}
          height={14}
          variant="mini"
          seed={(track.index ?? 1) * 2654435761} />
      </div>
    {:else if col.key === 'cover'}
      <div class="ff-trow__cell ff-trow__cell--cover" style={cellStyle(col)}>
        <div
          class="ff-trow__cover"
          style={track.cover ? `background-image:url(${track.cover})` : undefined}>
        </div>
      </div>
    {:else if col.key === 'key'}
      {@const k = String(track.key ?? '—')}
      <div class="ff-trow__cell ff-trow__cell--key" style={cellStyle(col)}>
        <span class="ff-trow__text" style="color:{k === '—' ? 'var(--ff-muted)' : keyColor(k)}">{k}</span>
      </div>
    {:else}
      <div class="ff-trow__cell ff-trow__cell--{col.key}" style={cellStyle(col)}>
        <span class="ff-trow__text">{track[col.key] ?? ''}</span>
      </div>
    {/if}
  {/each}
</div>
