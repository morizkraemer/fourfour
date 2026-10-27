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

  const SINGLE_META_KEYS = ['ALBUM', 'LABEL', 'BPM · KEY', 'DURATION', 'ADDED'];
  const MULTI_META_KEYS = ['TOTAL SIZE', 'TOTAL DURATION', 'BPM RANGE', 'KEYS'];
</script>

<script>
  import './detail-pane.css';
  import Waveform from './Waveform.svelte';
  import ScrubValue from './ScrubValue.svelte';
  import { bpmScrub, keyScrub } from '../utils/scrub.js';
  import { keyColor } from '../utils/key-color.js';
  import CueEditor from './CueEditor.svelte';
  import { middleTruncate } from '../utils/middle-truncate.js';

  let {
    mode = 'empty',
    title = '',
    artist = '',
    meta = {},
    cues = [],
    cuePalette = [],
    showCues = false,
    canSetCues = false,
    canJumpToCues = false,
    onCueAdd = undefined,
    onCueJump = undefined,
    onCueRename = undefined,
    onCueRecolor = undefined,
    onCueDelete = undefined,
    multiCount = 0,
    filePath = '',
    cover = '',
    peaks = null,
    bands = null,
    analyzing = false,
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

  // Overview-waveform cue dots, positioned along the track duration.
  let waveCues = $derived.by(() => {
    const totalSec = timeToSeconds(lookupKey(meta, 'DURATION'));
    if (!totalSec) return [];
    return (cues || []).map((cue) => ({ position: Math.min(1, cue.timeMs / 1000 / totalSec), color: cue.color }));
  });

  let hotChips = $derived((cues || []).filter((c) => c.slot > 0));
  let memoryChips = $derived((cues || []).filter((c) => c.slot === 0));
  let editingKey = $state(null);
  let editingChip = $derived((cues || []).find((c) => c.key === editingKey) ?? null);

  const ADD_BLOCKED = 'Load this track in the player to set a cue at the playhead';

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
          animating={analyzing && !bands && !peaks?.length}
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

    {#if !isMissing && showCues}
      {#each [{ title: 'HOT CUE', kind: 'hot', chips: hotChips, addLabel: 'Add hot cue at playhead' }, { title: 'MEMORY CUE', kind: 'memory', chips: memoryChips, addLabel: 'Add memory cue at playhead' }] as group (group.kind)}
        <div class="ff-detail-pane__cues-section" data-cue-group={group.kind}>
          <div class="ff-detail-pane__section-title">{group.title}</div>
          <div class="ff-detail-pane__cues-grid">
            {#each group.chips as chip (chip.key)}
              <div
                class="ff-detail-pane__cue-chip{editingKey === chip.key ? ' ff-detail-pane__cue-chip--active' : ''}"
                data-cue-key={chip.key}>
                <button
                  type="button"
                  class="ff-detail-pane__cue-main"
                  title={canJumpToCues ? `${chip.label}: jump here` : chip.label}
                  aria-label={chip.label}
                  disabled={!canJumpToCues}
                  onclick={() => onCueJump?.(chip)}
                  ondblclick={() => (editingKey = chip.key)}>
                  <span class="ff-detail-pane__cue-badge" style="background-color:{chip.color}">{chip.letter}</span>
                  {#if chip.name}<span class="ff-detail-pane__cue-name">{chip.name}</span>{/if}
                  <span class="ff-detail-pane__cue-pos">{chip.time}</span>
                </button>
                <button
                  type="button"
                  class="ff-detail-pane__cue-edit"
                  title="Rename, recolour or delete"
                  aria-label="Edit {chip.label}"
                  onclick={() => (editingKey = editingKey === chip.key ? null : chip.key)}>···</button>
              </div>
            {/each}
            <button
              type="button"
              class="ff-detail-pane__cue-chip ff-detail-pane__cue-chip--add"
              title={canSetCues ? group.addLabel : ADD_BLOCKED}
              aria-label={group.addLabel}
              disabled={!canSetCues}
              onclick={() => onCueAdd?.(group.kind)}>+</button>
          </div>
          {#if editingChip && (editingChip.slot === 0) === (group.kind === 'memory')}
            <div class="ff-detail-pane__cue-editor">
              <CueEditor
                chip={editingChip}
                palette={cuePalette}
                onRename={(name) => editingChip && onCueRename?.(editingChip, name)}
                onRecolor={(color) => editingChip && onCueRecolor?.(editingChip, color)}
                onDelete={() => {
                  if (editingChip) onCueDelete?.(editingChip);
                  editingKey = null;
                }}
                onClose={() => (editingKey = null)} />
            </div>
          {/if}
        </div>
      {/each}
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
