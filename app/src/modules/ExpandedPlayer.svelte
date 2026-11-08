<!--
  ExpandedPlayer.svelte — docked ~1/3-height player panel.

  Hosts the dual-view WaveformView (overview preview + Rekordbox playhead-locked
  zoom view). Carries the same transport as the compact bar (CUE, play, BPM, key,
  times) plus zoom controls for the waveform. Seeking routes through the same
  `seekToProgress` path as the compact strip, so playback/scrub behaviour matches.
-->
<script>
  import { WaveformView, CueEditor, Icon } from '$ds';
  import { nativeListener } from '$ds/components/nativeListener.js';
  import PlayerTempo from './PlayerTempo.svelte';
  import { isTrackAnalyzing } from '../stores/library.svelte.ts';
  import { cuesOf } from '../stores/cues.svelte.ts';
  import { altKey } from '../stores/alt-key.svelte.ts';
  import { quantize } from '../stores/quantize.svelte.ts';
  import { nearestBeatMs } from '../lib/quantize.ts';
  import { registerWaveZoom } from '../stores/wave-zoom.ts';
  import { padAction, padFace } from '../lib/padModifier.ts';
  import {
    addMemoryCueAtPlayhead,
    deleteCue,
    jumpToCue,
    deleteHotCue,
    moveHotCue,
    recolorCue,
    renameCue,
    triggerHotCue,
  } from '../stores/cue-actions.ts';
  import { gridNudgeMs } from '../lib/beatGrid.ts';
  import { getWaveformWindow } from '../services/tauri.svelte.ts';
  import { windowFromBytes } from '../services/waveform-bands.ts';
  import { CUE_PALETTE, HOT_CUE_SLOTS, cueChip, cueColor, cueKey, cueMarkers, formatCueTime, hotCueIn, memoryCues, slotLetter } from '../lib/cues.ts';
  import {
    player,
    loadZoomDetail,
    currentTime,
    currentTimeMs,
    totalTime,
    togglePlay,
    seekToProgress,
    cuePreviewStart,
    cuePreviewStop,
    toggleExpanded,
    beatGridNudgeLeft,
    beatGridNudgeRight,
    beatGridSetCurrent,
    beatGridShiftByBeat,
  } from '../stores/player.svelte.ts';

  let wave = $state(null);
  $effect(() => {
    if (wave) return registerWaveZoom(wave);
  });
  let hasBeats = $derived((player.track?.beats.length ?? 0) > 1);
  let windowTrackId = $derived(player.track?.bandsZoom ? player.track.id : null);
  let windowSource = $derived(
    windowTrackId == null
      ? null
      : async (startMs, endMs, columns) => windowFromBytes(await getWaveformWindow(windowTrackId, startMs, endMs, columns)),
  );
  const nudgeMs = (e) => gridNudgeMs(wave?.msPerPixel() ?? 0, e.altKey, wave?.msPerSample());

  $effect(() => {
    const id = player.track?.id;
    if (id != null) void loadZoomDetail(id);
  });
  let trackCues = $derived(cuesOf(player.track?.id));
  let markers = $derived(cueMarkers(trackCues, player.track?.durationMs ?? 0));
  let pads = $derived(
    Array.from({ length: HOT_CUE_SLOTS }, (_, i) => {
      const cue = hotCueIn(trackCues, i + 1);
      return { slot: i + 1, letter: slotLetter(i + 1), chip: cue ? cueChip(cue) : null };
    }),
  );
  let memory = $derived(memoryCues(trackCues));
  let cueSnap = $derived(quantize.on ? (ms) => nearestBeatMs(player.track?.beats ?? [], ms) : null);
  const moveCue = (key, ms) => player.track && moveHotCue(player.track.id, key, ms);
  let editingSlot = $state(null);
  let editingChip = $derived(pads.find((p) => p.slot === editingSlot)?.chip ?? null);

  function firePad(pad, altHeld) {
    if (padAction(altHeld, !!pad.chip) === 'delete') {
      if (editingSlot === pad.slot) editingSlot = null;
      deleteHotCue(pad.slot);
    } else {
      triggerHotCue(pad.slot);
    }
  }

  function pressMemoryChip(cue, altHeld) {
    if (!player.track) return;
    if (altHeld) deleteCue(player.track.id, cueKey(cue));
    else void jumpToCue(cue);
  }

  function openEditor(e, pad) {
    e.preventDefault();
    if (pad.chip) editingSlot = editingSlot === pad.slot ? null : pad.slot;
  }

  const stop = (fn) => (e) => {
    e.stopPropagation();
    fn?.(e);
  };
</script>

<div class="ff-xp" role="group" aria-label="Expanded player">
  <header class="ff-xp__head">
    <div class="ff-xp__meta">
      {#if player.cover}
        <img class="ff-xp__cover" src={player.cover} alt="" />
      {:else}
        <div class="ff-xp__cover ff-xp__cover--empty"></div>
      {/if}
      <div class="ff-xp__titles">
        <span class="ff-xp__title">{player.track?.title || 'No Track Loaded'}</span>
        <span class="ff-xp__artist">{player.track?.artist || '—'}</span>
      </div>
    </div>

    <div class="ff-xp__head-right">
      <PlayerTempo expanded />
      <div class="ff-xp__times">
        <span class="ff-xp__time-cur">{currentTime()}<span class="ff-xp__time-ms">{currentTimeMs()}</span></span>
        <span class="ff-xp__time-tot">{totalTime()}</span>
      </div>
      <button
        class="ff-xp__collapse"
        title="Collapse player (Esc)"
        aria-label="Collapse player"
        onclick={toggleExpanded}>
        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor"
          stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="6 9 12 15 18 9" />
        </svg>
      </button>
    </div>
  </header>

  <div class="ff-xp__wave">
    <WaveformView
      bind:this={wave}
      mode="expanded"
      bands={player.track?.bands ?? null}
      bandsDetail={player.track?.bandsDetail ?? null}
      bandsOverview={player.track?.bandsOverview ?? null}
      bandsZoom={player.track?.bandsZoom ?? null}
      detailPending={player.track != null && player.zoomDetailLoadingId === player.track.id}
      {windowSource}
      cues={markers}
      beats={player.track?.beats ?? []}
      bpm={player.track?.bpm ?? 0}
      durationMs={player.track?.durationMs ?? 0}
      trackKey={player.track?.id ?? null}
      analyzing={player.track ? isTrackAnalyzing(player.track.id) : false}
      progress={player.progress}
      playing={player.playing}
      onSeek={player.track ? seekToProgress : undefined}
      onCueMove={moveCue}
      {cueSnap} />
  </div>

  <footer class="ff-xp__transport">
    <div class="ff-xp__transport-left">
      <button
        class="ff-xp__cue-btn"
        title="Cue (hold to preview)"
        aria-label="Cue"
        disabled={!player.track}
        {@attach nativeListener('pointerdown', stop(cuePreviewStart))}
        {@attach nativeListener('pointerup', stop(cuePreviewStop))}
        onpointercancel={stop(cuePreviewStop)}
        onpointerleave={stop(cuePreviewStop)}>
        <span class="ff-xp__cue-txt">CUE</span>
      </button>
      <button
        class="ff-xp__play"
        title={player.playing ? 'Pause' : 'Play'}
        aria-label={player.playing ? 'Pause' : 'Play'}
        disabled={!player.track}
        onclick={() => togglePlay()}>
        {#if player.playing}
          <svg viewBox="0 0 24 24" width="13" height="13" fill="currentColor"><rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/></svg>
        {:else}
          <svg viewBox="0 0 24 24" width="13" height="13" fill="currentColor"><polygon points="6,4 6,20 18,12"/></svg>
        {/if}
      </button>
    </div>

    <div class="ff-xp__pads" role="group" aria-label="Hot cues">
      {#each pads as pad (pad.slot)}
        <button
          class="ff-xp__pad{pad.chip ? ' ff-xp__pad--set' : ''}{pad.chip && altKey.held ? ' ff-xp__pad--remove' : ''}"
          style:--pad-color={pad.chip?.color}
          title={pad.chip
            ? `${pad.chip.label}: jump and play (right-click to rename, recolour or delete)`
            : `Hot cue ${pad.letter}: set at the playhead`}
          aria-label={pad.chip ? `Hot cue ${pad.chip.label}` : `Set hot cue ${pad.letter}`}
          data-pad={pad.letter}
          disabled={!player.track}
          onpointerdown={(e) => e.button === 0 && firePad(pad, e.altKey)}
          onclick={(e) => e.detail === 0 && firePad(pad, e.altKey)}
          oncontextmenu={(e) => openEditor(e, pad)}>{padFace(pad.letter, !!pad.chip, altKey.held)}</button>
      {/each}
      {#if editingChip && player.track}
        <div class="ff-xp__cue-editor">
          <CueEditor
            chip={editingChip}
            palette={CUE_PALETTE}
            onRename={(name) => editingChip && player.track && renameCue(player.track.id, editingChip.key, name)}
            onRecolor={(color) => editingChip && player.track && recolorCue(player.track.id, editingChip.key, color)}
            onDelete={() => {
              if (editingChip && player.track) deleteCue(player.track.id, editingChip.key);
              editingSlot = null;
            }}
            onClose={() => (editingSlot = null)} />
        </div>
      {/if}
    </div>

    <div class="ff-xp__memory" role="group" aria-label="Memory cues">
      <span class="ff-xp__label">Memory</span>
      <div class="ff-xp__chips">
        {#each memory as cue (cueKey(cue))}
          <button
            class="ff-xp__chip{altKey.held ? ' ff-xp__chip--remove' : ''}"
            type="button"
            data-memory-chip
            title={altKey.held ? 'Remove this memory cue' : `Jump to ${formatCueTime(cue.timeMs)} (Alt-click removes)`}
            onclick={(e) => pressMemoryChip(cue, e.altKey)}>
            <i class="ff-xp__dot" style:background={cueColor(cue)}></i>
            <span class="ff-xp__chip-time">{formatCueTime(cue.timeMs)}</span>
            {#if cue.name}<span class="ff-xp__chip-name">{cue.name}</span>{/if}
          </button>
        {/each}
        <button class="ff-xp__pad ff-xp__pad--memory" type="button" title="Add a memory cue at the playhead (M)" aria-label="Add memory cue"
          disabled={!player.track} onclick={() => addMemoryCueAtPlayhead()}><Icon name="plus" size={12} /></button>
      </div>
    </div>

    <div class="ff-xp__tools">
      <div class="ff-xp__group">
        <span class="ff-xp__label">Grid</span>
        <div class="ff-xp__box" role="group" aria-label="Beat grid">
          <button class="ff-xp__c" title="Shift grid one beat earlier" aria-label="Shift grid one beat earlier"
            disabled={!hasBeats} onclick={() => beatGridShiftByBeat(-1)}>«</button>
          <button class="ff-xp__c" title="Nudge grid left one pixel (Alt: 1 sample)" aria-label="Nudge grid left"
            disabled={!hasBeats} onclick={(e) => beatGridNudgeLeft(nudgeMs(e))}>‹</button>
          <button class="ff-xp__c ff-xp__c--wide" title="Set downbeat to playhead" aria-label="Set downbeat to playhead"
            disabled={!hasBeats} onclick={() => beatGridSetCurrent()}>Set 1</button>
          <button class="ff-xp__c" title="Nudge grid right one pixel (Alt: 1 sample)" aria-label="Nudge grid right"
            disabled={!hasBeats} onclick={(e) => beatGridNudgeRight(nudgeMs(e))}>›</button>
          <button class="ff-xp__c" title="Shift grid one beat later" aria-label="Shift grid one beat later"
            disabled={!hasBeats} onclick={() => beatGridShiftByBeat(1)}>»</button>
        </div>
      </div>
      <div class="ff-xp__box" role="group" aria-label="Zoom">
        <button class="ff-xp__c" title="Zoom out" aria-label="Zoom out" disabled={!player.track} onclick={() => wave?.zoomOut()}><Icon name="minus" size={11} /></button>
        <button class="ff-xp__c ff-xp__c--wide" title="Default zoom" aria-label="Default zoom" disabled={!player.track} onclick={() => wave?.resetZoom()}>Fit</button>
        <button class="ff-xp__c" title="Zoom in" aria-label="Zoom in" disabled={!player.track} onclick={() => wave?.zoomIn()}><Icon name="plus" size={11} /></button>
      </div>
    </div>
  </footer>
</div>

<style>
  .ff-xp {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    min-height: 0;
    background: var(--ff-bg, #0d0d0d);
  }

  .ff-xp__head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--ff-space-4, 16px);
    padding: 10px 14px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    flex-shrink: 0;
  }

  .ff-xp__meta {
    display: flex;
    align-items: center;
    gap: var(--ff-space-3, 12px);
    min-width: 0;
  }
  .ff-xp__cover {
    width: 40px;
    height: 40px;
    border-radius: var(--ff-radius-sm, 4px);
    object-fit: cover;
    flex-shrink: 0;
  }
  .ff-xp__cover--empty { background: var(--ff-elev, rgba(255, 255, 255, 0.06)); }
  .ff-xp__titles { display: flex; flex-direction: column; min-width: 0; gap: 1px; }
  .ff-xp__title {
    font-weight: 500;
    font-size: 13px;
    color: var(--ff-text, #fff);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .ff-xp__artist {
    font-size: 11.5px;
    color: var(--ff-muted, rgba(255, 255, 255, 0.6));
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }

  .ff-xp__head-right { display: flex; align-items: center; gap: var(--ff-space-8); }
  .ff-xp__times {
    display: grid;
    justify-items: end;
    row-gap: 3px;
    min-width: 9ch;
    font-family: var(--ff-font-mono);
    font-variant-numeric: tabular-nums;
    line-height: 1;
    white-space: nowrap;
  }
  .ff-xp__time-cur {
    color: var(--ff-text);
    font-size: 13px;
  }
  .ff-xp__time-ms {
    color: var(--ff-muted);
    font-size: 11px;
  }
  .ff-xp__time-tot {
    color: var(--ff-muted);
    font-size: 11px;
  }

  .ff-xp__collapse {
    display: inline-flex; align-items: center; justify-content: center;
    width: 30px; height: 30px;
    border: none; border-radius: var(--ff-radius-sm, 4px);
    background: transparent; color: var(--ff-muted, rgba(255, 255, 255, 0.6));
    cursor: pointer;
  }
  .ff-xp__collapse:hover { background: var(--ff-elev, rgba(255, 255, 255, 0.06)); color: var(--ff-text, #fff); }

  .ff-xp__wave { flex: 1; min-height: 0; position: relative; }

  .ff-xp__transport {
    display: flex; align-items: center; justify-content: flex-start;
    gap: var(--ff-space-8);
    padding: 10px 14px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    flex-shrink: 0;
    position: relative;
  }
  .ff-xp__transport-left { display: flex; align-items: center; gap: var(--ff-space-4); }

  /* CUE / play match the compact player: circular, orange/green strokes. */
  .ff-xp__cue-btn {
    display: inline-flex; align-items: center; justify-content: center;
    width: 28px; height: 28px; padding: 0;
    border: 1px solid #f59e0b; border-radius: 50%;
    background: transparent; cursor: pointer;
    transition: background-color var(--ff-motion-fast, 120ms) ease;
  }
  .ff-xp__cue-btn:hover:not(:disabled) { background-color: rgba(245, 158, 11, 0.08); }
  .ff-xp__cue-btn:disabled { opacity: 0.4; cursor: default; }
  .ff-xp__cue-txt {
    font-family: var(--ff-font-mono, monospace);
    font-size: 8px; font-weight: 500; color: #d4d4d4; line-height: 1;
  }

  .ff-xp__play {
    display: inline-flex; align-items: center; justify-content: center;
    width: 28px; height: 28px; padding: 0;
    border: 1px solid #22c55e; border-radius: 50%;
    background: transparent; color: #d4d4d4; cursor: pointer;
    transition: background-color var(--ff-motion-fast, 120ms) ease;
  }
  .ff-xp__play:hover:not(:disabled) { background-color: rgba(34, 197, 94, 0.08); }
  .ff-xp__play:disabled { opacity: 0.4; cursor: default; }

  /* Hot cue pads A-H: filled with the cue colour once set. */
  .ff-xp__pads { display: flex; align-items: center; gap: 4px; position: relative; }
  .ff-xp__pad {
    height: 28px; min-width: 28px; padding: 0 6px;
    border: 1px solid var(--ff-border-hi, rgba(255, 255, 255, 0.18));
    border-radius: var(--ff-radius-sm, 4px);
    background: transparent; color: var(--ff-muted, rgba(255, 255, 255, 0.5));
    font-family: var(--ff-font-mono, monospace); font-size: 11px; font-weight: 700; line-height: 1;
    cursor: pointer;
  }
  .ff-xp__pad:hover:not(:disabled) { background: var(--ff-elev, rgba(255, 255, 255, 0.08)); color: var(--ff-text, #fff); }
  .ff-xp__pad--set { background: var(--pad-color); border-color: var(--pad-color); color: #0a0a0a; }
  .ff-xp__pad--set:hover:not(:disabled) { background: var(--pad-color); color: #0a0a0a; filter: brightness(1.12); }
  .ff-xp__pad--remove { opacity: 0.7; }
  .ff-xp__pad--memory { flex: none; color: var(--ff-danger); border-color: var(--ff-danger-border-hi); }
  .ff-xp__pad:disabled { opacity: 0.4; cursor: default; }
  .ff-xp__cue-editor {
    position: absolute; bottom: calc(100% + 12px); left: 0; width: 264px; z-index: 5;
  }

  .ff-xp__label {
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-label);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ff-muted);
    line-height: 1;
    user-select: none;
  }
  .ff-xp__group { display: inline-flex; align-items: center; gap: var(--ff-space-4); }
  .ff-xp__box {
    display: inline-flex;
    align-items: stretch;
    height: 28px;
    box-sizing: border-box;
    border: 1px solid var(--ff-border-hi);
    border-radius: var(--ff-radius-md);
    background: var(--ff-bg);
    overflow: hidden;
  }
  .ff-xp__box > * + * { border-left: 1px solid var(--ff-border); }
  .ff-xp__c {
    display: inline-flex; align-items: center; justify-content: center;
    min-width: 26px; padding: 0 var(--ff-space-4);
    border: none; background: transparent; color: var(--ff-text-mid);
    font-family: var(--ff-font); font-size: var(--ff-type-compact); font-weight: 500; line-height: 1;
    cursor: pointer;
  }
  .ff-xp__c--wide { padding: 0 var(--ff-space-5); }
  .ff-xp__c:hover:not(:disabled) { background: var(--ff-elev-hi); color: var(--ff-text); }
  .ff-xp__c:active:not(:disabled) { background: var(--ff-select); }
  .ff-xp__c:disabled { opacity: 0.4; cursor: default; }

  .ff-xp__memory {
    flex: 1; min-width: 0; align-self: stretch;
    display: flex; align-items: center; gap: var(--ff-space-4);
    padding-left: var(--ff-space-8);
    border-left: 1px solid var(--ff-border);
  }
  .ff-xp__chips {
    display: flex; align-items: center; gap: var(--ff-space-2);
    min-width: 0; overflow-x: auto; scrollbar-width: none;
  }
  .ff-xp__chip {
    display: inline-flex; align-items: center; flex: none; gap: var(--ff-space-3);
    height: 28px; padding: 0 var(--ff-space-4); box-sizing: border-box;
    border: 1px solid var(--ff-border-hi); border-radius: var(--ff-radius-md);
    background: transparent; color: var(--ff-text-mid);
    font: 500 var(--ff-type-compact) var(--ff-font);
    white-space: nowrap; cursor: pointer;
  }
  .ff-xp__chip:hover { background: var(--ff-elev-hi); color: var(--ff-text); }
  .ff-xp__chip--remove { opacity: 0.7; }
  .ff-xp__dot { width: 6px; height: 6px; border-radius: 50%; flex: none; }
  .ff-xp__chip-time { font-family: var(--ff-font-mono); font-size: var(--ff-type-num); font-variant-numeric: tabular-nums; }
  .ff-xp__chip-name { color: var(--ff-muted); }

  .ff-xp__tools { display: flex; align-items: center; gap: var(--ff-space-8); margin-left: auto; }
</style>
