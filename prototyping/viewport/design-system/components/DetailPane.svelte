<script module>
  // Detail pane (Svelte port of detail-pane.js). The morphing right panel in
  // Browse: empty / single / missing / multi modes.
  function lookupKey(obj, key) {
    if (!obj) return undefined;
    if (obj[key] !== undefined) return obj[key];
    if (obj[key.toLowerCase()] !== undefined) return obj[key.toLowerCase()];
    const found = Object.keys(obj).find((k) => k.toLowerCase() === key.toLowerCase());
    return found ? obj[found] : undefined;
  }

  // "m:ss" → seconds; null if unparseable.
  function timeToSeconds(str) {
    const m = /^(\d+):(\d{1,2})$/.exec(String(str ?? '').trim());
    if (!m) return null;
    return Number(m[1]) * 60 + Number(m[2]);
  }

  // Cue color palette matching the track-row / cuestrip CDJ colors.
  const CUE_COLORS = ['#ec4899', '#38bdf8', '#4ade80', '#f59e0b', '#a78bfa', '#facc15', '#94a3b8'];

  const SINGLE_META_KEYS = ['ALBUM', 'LABEL', 'BPM · KEY', 'DURATION', 'ADDED'];
  const MULTI_META_KEYS = ['TOTAL SIZE', 'TOTAL DURATION', 'BPM RANGE', 'KEYS'];
</script>

<script>
  import './detail-pane.css';
  import Waveform from './Waveform.svelte';
  import ScrubValue from './ScrubValue.svelte';
  import { bpmScrub, keyScrub } from '../utils/scrub.js';
  import { keyColor } from '../utils/key-color.js';
  import { middleTruncate } from '../utils/middle-truncate.js';

  let {
    mode = 'empty',
    title = '',
    artist = '',
    meta = {},
    cues = [],
    multiCount = 0,
    filePath = '',
    cover = '',
    peaks = null,
    bands = null,
    onWaveformSeek = undefined,
    onRevealPath = undefined,
    onPathContextMenu = undefined,
    bpm = 0,
    musicalKey = '',
    onBpmChange = undefined,
    onKeyChange = undefined,
    bpmMenuItems = [],
    keyMenuItems = [],
  } = $props();

  let tempoEditable = $derived(!!onBpmChange && !!onKeyChange);

  let isMissing = $derived(mode === 'missing');
  let stripWidth = $state(0);

  // Overview-waveform hot-cue dots, positioned along the track duration.
  let waveCues = $derived.by(() => {
    const totalSec = timeToSeconds(lookupKey(meta, 'DURATION'));
    return (cues || []).map((cue, idx) => {
      const sec = timeToSeconds(cue.position);
      const frac = totalSec && sec != null ? sec / totalSec : cues.length > 1 ? idx / (cues.length - 1) : 0;
      return { position: frac, color: cue.color || CUE_COLORS[idx % CUE_COLORS.length] };
    });
  });

  // Single/missing meta rows, with the BPM · KEY composite row.
  let singleMeta = $derived(
    SINGLE_META_KEYS.map((key) => {
      let val = '—';
      if (!isMissing) {
        if (key === 'BPM · KEY') {
          const bpmVal = lookupKey(meta, 'BPM') || '—';
          const keyVal = lookupKey(meta, 'KEY') || '—';
          val = bpmVal !== '—' || keyVal !== '—' ? `${bpmVal} · ${keyVal}` : '—';
        } else {
          val = lookupKey(meta, key) || '—';
        }
      }
      return { key, val };
    })
  );

  let multiMeta = $derived(MULTI_META_KEYS.map((key) => ({ key, val: lookupKey(meta, key) || '—' })));

  let displayPath = $derived(
    filePath || (isMissing ? '/Volumes/Music/Tale Of Us/Endless Run/02 Voidwalker.flac' : '')
  );

  let pathEl = $state(null);
  let pathMaxChars = $state(Infinity);

  $effect(() => {
    const el = pathEl;
    if (!el) return;
    const measure = () => {
      const style = getComputedStyle(el);
      const ctx = document.createElement('canvas').getContext('2d');
      if (!ctx) return;
      ctx.font = `${style.fontSize} ${style.fontFamily}`;
      const charWidth = ctx.measureText('0').width;
      const innerWidth = el.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
      pathMaxChars = charWidth > 0 ? Math.max(1, Math.floor(innerWidth / charWidth)) : Infinity;
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  });

  let shownPath = $derived(middleTruncate(displayPath, pathMaxChars));
</script>

<div class="ff-detail-pane">
  {#if mode === 'empty'}
    <div class="ff-detail-pane__empty-state">
      <svg
        class="ff-detail-pane__empty-icon"
        viewBox="0 0 24 24"
        width="40"
        height="40"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round">
        <circle cx="12" cy="12" r="10" /><circle cx="12" cy="12" r="2" />
      </svg>
      <p class="ff-detail-pane__empty-label">No track selected</p>
      <p class="ff-detail-pane__empty-sub">Select a track to view details</p>
    </div>
  {:else if mode === 'single' || mode === 'missing'}
    <div class="ff-detail-pane__cover{isMissing ? ' ff-detail-pane__cover--missing' : ''}">
      {#if cover && cover !== '' && cover !== 'undefined' && cover !== 'null'}
        <img class="ff-detail-pane__cover-image" src={cover} alt="Cover Art" />
      {:else}
        <svg
          class="ff-detail-pane__art-icon"
          viewBox="0 0 24 24"
          width="146"
          height="146"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round">
          <path d="M9 18V5l12-2v13" /><circle cx="6" cy="18" r="3" /><circle cx="18" cy="16" r="3" />
        </svg>
      {/if}
    </div>

    {#if !isMissing}
      <div class="ff-detail-pane__waveform" bind:clientWidth={stripWidth}>
        <Waveform
          {bands}
          {peaks}
          width={stripWidth || 300}
          height={36}
          variant="strip"
          look="bands"
          cues={waveCues}
          cueStyle="dot"
          onSeek={onWaveformSeek} />
      </div>
    {/if}

    <div class="ff-detail-pane__header">
      <h2 class="ff-detail-pane__title">{title || 'Untitled Track'}</h2>
      <p class="ff-detail-pane__artist">{artist || 'Unknown Artist'}</p>
    </div>

    {#if isMissing}
      <div class="ff-detail-pane__warn-alert">File not found on disk</div>
    {/if}

    <dl class="ff-detail-pane__meta-list">
      {#each singleMeta as row}
        <dt class="ff-detail-pane__dt">{row.key}</dt>
        {#if row.key === 'BPM · KEY' && tempoEditable && !isMissing}
          <dd class="ff-detail-pane__dd ff-detail-pane__dd--tempo">
            <ScrubValue model={bpmScrub} value={bpm} label="BPM" onCommit={onBpmChange} extraItems={bpmMenuItems} />
            <span class="ff-detail-pane__tempo-sep" aria-hidden="true">·</span>
            <ScrubValue model={keyScrub} value={musicalKey} label="Key" color={keyColor(musicalKey)} onCommit={onKeyChange} extraItems={keyMenuItems} />
          </dd>
        {:else}
          <dd class="ff-detail-pane__dd">{row.val}</dd>
        {/if}
      {/each}
    </dl>

    {#if !isMissing && cues && cues.length > 0}
      <div class="ff-detail-pane__cues-section">
        <div class="ff-detail-pane__section-title">HOT CUE</div>
        <div class="ff-detail-pane__cues-grid">
          {#each cues as cue, idx}
            {@const color = cue.color || CUE_COLORS[idx % CUE_COLORS.length]}
            <div class="ff-detail-pane__cue-chip{idx === 0 ? ' ff-detail-pane__cue-chip--active' : ''}">
              <div class="ff-detail-pane__cue-dot" style="background-color:{color}"></div>
              <span class="ff-detail-pane__cue-name">{cue.name}</span>
              <span class="ff-detail-pane__cue-pos">{cue.position}</span>
            </div>
          {/each}
          <div class="ff-detail-pane__cue-chip ff-detail-pane__cue-chip--add">+</div>
        </div>
      </div>
    {/if}

    {#if displayPath}
      <div class="ff-detail-pane__file-path-section">
        <div class="ff-detail-pane__section-title">FILE PATH</div>
        <button
          type="button"
          bind:this={pathEl}
          class="ff-detail-pane__file-path-val"
          title={displayPath}
          onclick={() => onRevealPath?.()}
          oncontextmenu={(e) => {
            if (!onPathContextMenu) return;
            e.preventDefault();
            onPathContextMenu(e);
          }}
        >{shownPath}</button>
      </div>
    {/if}

    {#if isMissing}
      <div class="ff-detail-pane__actions">
        <button class="ff-detail-pane__action-link">Locate file</button>
        <button class="ff-detail-pane__action-link">Reanalyze</button>
        <button class="ff-detail-pane__action-link ff-detail-pane__action-link--destructive">Remove</button>
      </div>
    {/if}
  {:else if mode === 'multi'}
    <div class="ff-detail-pane__count-display">{multiCount}</div>
    <div class="ff-detail-pane__count-label">tracks selected</div>
    <dl class="ff-detail-pane__meta-list">
      {#each multiMeta as row}
        <dt class="ff-detail-pane__dt">{row.key}</dt>
        <dd class="ff-detail-pane__dd">{row.val}</dd>
      {/each}
    </dl>
  {/if}
</div>
