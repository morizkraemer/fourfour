<script>
  import './detail-pane.css';
  import './inspector.css';
  import Waveform from './Waveform.svelte';
  import ScrubValue from './ScrubValue.svelte';
  import GhostField from './GhostField.svelte';
  import LockMark from './LockMark.svelte';
  import CueEditor from './CueEditor.svelte';
  import { bpmScrub, keyScrub } from '../utils/scrub.js';
  import { keyColor } from '../utils/key-color.js';
  import { middleTruncate } from '../utils/middle-truncate.js';

  let {
    mode = 'empty',
    title = '',
    artist = '',
    cover = '',
    fields = [],
    onFieldChange = undefined,
    duration = '',
    bpm = 0,
    musicalKey = '',
    bpmLocked = false,
    keyLocked = false,
    onBpmChange = undefined,
    onKeyChange = undefined,
    onReleaseLocks = undefined,
    bpmMenuItems = [],
    keyMenuItems = [],
    bands = null,
    peaks = null,
    analyzing = false,
    markers = [],
    onWaveformSeek = undefined,
    onMarkerClick = undefined,
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
    multiMeta = {},
    filePath = '',
    onRevealPath = undefined,
    onPathContextMenu = undefined,
  } = $props();

  let tempoEditable = $derived(!!onBpmChange && !!onKeyChange);
  let waveWidth = $state(0);
  let editingKey = $state(null);
  let editingChip = $derived(cues.find((c) => c.key === editingKey) ?? null);
  let hotCount = $derived(cues.filter((c) => c.slot > 0).length);
  let memoryCount = $derived(cues.length - hotCount);
  let canEditFields = $derived(!!onFieldChange);

  let lockText = $derived(
    bpmLocked && keyLocked ? 'BPM and key are hand-set' : bpmLocked ? 'BPM is hand-set' : keyLocked ? 'Key is hand-set' : ''
  );

  const ADD_BLOCKED = 'Load this track in the player to set a cue at the playhead';

  let multiRows = $derived(Object.entries(multiMeta));

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
      pathMaxChars = charWidth > 0 ? Math.max(1, Math.floor(el.clientWidth / charWidth)) : Infinity;
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  });

  let shownPath = $derived(middleTruncate(filePath, pathMaxChars));

  function markerLeft(position) {
    return `clamp(7px, ${(position * 100).toFixed(3)}%, calc(100% - 7px))`;
  }
</script>

<div class="ff-detail-pane ff-inspector">
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
  {:else if mode === 'single'}
    <div class="ff-inspector__id">
      <div class="ff-inspector__cover">
        {#if cover && cover !== 'undefined' && cover !== 'null'}
          <img class="ff-inspector__cover-image" src={cover} alt="Cover Art" />
        {:else}
          <svg
            viewBox="0 0 24 24"
            width="36"
            height="36"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round">
            <path d="M9 18V5l12-2v13" /><circle cx="6" cy="18" r="3" /><circle cx="18" cy="16" r="3" />
          </svg>
        {/if}
      </div>
      <div class="ff-inspector__idf">
        {#if canEditFields}
          <GhostField title value={title} label="Title" placeholder="Untitled Track" onCommit={(v) => onFieldChange('title', v)} />
          <GhostField value={artist} label="Artist" placeholder="Unknown Artist" onCommit={(v) => onFieldChange('artist', v)} />
        {:else}
          <h2 class="ff-inspector__title" title={title}>{title || 'Untitled Track'}</h2>
          <p class="ff-inspector__artist" title={artist}>{artist || 'Unknown Artist'}</p>
        {/if}
      </div>
    </div>

    <div class="ff-inspector__figs">
      <div class="ff-inspector__fig" data-fig="bpm">
        <span class="ff-inspector__fig-label">BPM</span>
        <div class="ff-inspector__fig-val">
          {#if tempoEditable}
            <ScrubValue model={bpmScrub} value={bpm} label="BPM" onCommit={onBpmChange} extraItems={bpmMenuItems} />
          {:else}
            <span class="ff-inspector__fig-static">{bpm ? Number(bpm).toFixed(1) : '—'}</span>
          {/if}
        </div>
      </div>
      <div class="ff-inspector__fig" data-fig="key">
        <span class="ff-inspector__fig-label">KEY</span>
        <div class="ff-inspector__fig-val">
          {#if tempoEditable}
            <ScrubValue model={keyScrub} value={musicalKey} label="Key" color={keyColor(musicalKey)} onCommit={onKeyChange} extraItems={keyMenuItems} />
          {:else}
            <span class="ff-inspector__fig-static" style:color={keyColor(musicalKey)}>{musicalKey || '—'}</span>
          {/if}
        </div>
      </div>
      <div class="ff-inspector__fig" data-fig="length">
        <span class="ff-inspector__fig-label">LENGTH</span>
        <div class="ff-inspector__fig-val"><span class="ff-inspector__fig-static">{duration || '—'}</span></div>
      </div>
    </div>

    {#if lockText}
      <div class="ff-inspector__lock">
        <LockMark locked title="{lockText}. Re-analysis keeps them. Click to hand them back to analysis." onToggle={() => onReleaseLocks?.()} />
        <span>{lockText}</span>
      </div>
    {/if}

    <div class="ff-inspector__wave" bind:clientWidth={waveWidth}>
      <Waveform
        {bands}
        {peaks}
        width={waveWidth || 300}
        height={56}
        variant="strip"
        look="bands"
        animating={analyzing && !bands && !peaks?.length}
        onSeek={onWaveformSeek} />
      {#each markers as marker (marker.key)}
        <button
          type="button"
          class="ff-inspector__marker{marker.label ? '' : ' ff-inspector__marker--memory'}"
          style:left={markerLeft(marker.position)}
          style:--cue={marker.color}
          data-marker-key={marker.key}
          title={marker.label ? `Cue ${marker.label}` : 'Memory cue'}
          aria-label={marker.label ? `Jump to cue ${marker.label}` : 'Jump to memory cue'}
          onclick={() => onMarkerClick?.(marker)}>{marker.label}</button>
      {/each}
    </div>

    {#if showCues}
      <section class="ff-inspector__cues" data-cue-list>
        <div class="ff-inspector__cues-head">
          <span>Cues <em>{hotCount} hot · {memoryCount} memory</em></span>
          <span class="ff-inspector__cues-add">
            <button type="button" disabled={!canSetCues} title={canSetCues ? 'Add hot cue at playhead' : ADD_BLOCKED} aria-label="Add hot cue at playhead" onclick={() => onCueAdd?.('hot')}>+ Hot</button>
            <button type="button" disabled={!canSetCues} title={canSetCues ? 'Add memory cue at playhead' : ADD_BLOCKED} aria-label="Add memory cue at playhead" onclick={() => onCueAdd?.('memory')}>+ Memory</button>
          </span>
        </div>
        {#each cues as chip (chip.key)}
          <div class="ff-inspector__cue{editingKey === chip.key ? ' ff-inspector__cue--active' : ''}" data-cue-key={chip.key}>
            <button
              type="button"
              class="ff-inspector__cue-main"
              title={canJumpToCues ? `${chip.label}: jump here` : chip.label}
              aria-label={chip.label}
              disabled={!canJumpToCues}
              onclick={() => onCueJump?.(chip)}
              ondblclick={() => (editingKey = chip.key)}>
              <span class="ff-inspector__cue-badge" style="background-color:{chip.color}">{chip.letter}</span>
              <span class="ff-inspector__cue-name{chip.name ? '' : ' ff-inspector__cue-name--empty'}">{chip.name || (chip.slot === 0 ? 'Memory cue' : 'Unnamed')}</span>
              <span class="ff-inspector__cue-pos">{chip.time}</span>
            </button>
            <button
              type="button"
              class="ff-inspector__cue-edit"
              title="Rename, recolour or delete"
              aria-label="Edit {chip.label}"
              onclick={() => (editingKey = editingKey === chip.key ? null : chip.key)}>···</button>
          </div>
          {#if editingChip && editingChip.key === chip.key}
            <div class="ff-inspector__cue-editor">
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
        {/each}
      </section>
    {/if}

    {#if fields.length}
      <div class="ff-inspector__rows">
        {#each fields as field (field.key)}
          <span class="ff-inspector__row-label{field.multiline ? ' ff-inspector__row-label--top' : ''}">{field.label}</span>
          {#if canEditFields && !field.readOnly}
            <GhostField
              value={field.value}
              mono={field.mono}
              multiline={field.multiline}
              label={field.label}
              placeholder={field.placeholder ?? ''}
              onCommit={(v) => onFieldChange(field.key, v)} />
          {:else}
            <span class="ff-inspector__row-val{field.mono ? ' ff-inspector__row-val--mono' : ''}" title={field.value}>{field.value || '—'}</span>
          {/if}
        {/each}
      </div>
    {/if}

    {#if filePath}
      <button
        type="button"
        bind:this={pathEl}
        class="ff-inspector__file-path-val"
        title={filePath}
        onclick={() => onRevealPath?.()}
        oncontextmenu={(e) => {
          if (!onPathContextMenu) return;
          e.preventDefault();
          onPathContextMenu(e);
        }}
      >{shownPath}</button>
    {/if}
  {:else if mode === 'multi'}
    <div class="ff-detail-pane__count-display">{multiCount}</div>
    <div class="ff-detail-pane__count-label">tracks selected</div>
    <dl class="ff-detail-pane__meta-list">
      {#each multiRows as [key, val]}
        <dt class="ff-detail-pane__dt">{key}</dt>
        <dd class="ff-detail-pane__dd">{val || '—'}</dd>
      {/each}
    </dl>
  {/if}
</div>
