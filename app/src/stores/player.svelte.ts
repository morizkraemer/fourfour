/**
 * player.svelte.ts — Playback state for the compact player bar.
 */

import {
  getAnalysisData,
  invalidateAnalysisCache,
  listenToAnalysisProgress,
  peekAnalysisData,
} from '../services/tauri.svelte.ts';
import {
  isValidColorWaveform,
  previewBytesFromPayload,
} from '../services/analysis-data.ts';
import {
  playbackPause,
  playbackPlay,
  playbackSeek,
  playbackSetPosition,
  playbackStop,
  subscribePlaybackTick,
} from '../services/playback.svelte.ts';
import { library, requestBackgroundAnalysis } from './library.svelte.ts';

export interface PlayerCue {
  position: number;
  color: string;
  timeMs: number;
}

export interface ColorWaveSample {
  amp: number;
  r: number;
  g: number;
  b: number;
}

export interface BeatMarker {
  time_ms: number;
  bar_position: number;
}

export interface PlayerTrack {
  id: number;
  sourcePath: string;
  title: string;
  artist: string;
  cover: string;
  bpm: string;
  key: string;
  totalSeconds: number;
  durationMs: number;
  cues: PlayerCue[];
  beats: BeatMarker[];
  colorData: ColorWaveSample[] | null;
  // Higher-res detail + fixed overview for the expanded dual-view player.
  // (compact strip uses colorData; these stay null until analysis loads.)
  colorDetail: ColorWaveSample[] | null;
  colorOverview: ColorWaveSample[] | null;
  previewBytes: number[] | null;
}

const CUE_COLORS = ['#4ade80', '#60a5fa', '#f472b6', '#fbbf24', '#a78bfa', '#fb7185', '#34d399', '#38bdf8'];

export const player = $state({
  playing: false,
  progress: 0.0,
  track: null as PlayerTrack | null,
  loading: false,
  expanded: false,
  levelLeft: 0,
  levelRight: 0,
});

/** Toggle the full-screen expanded player (dual-view waveform). */
export function toggleExpanded() {
  player.expanded = !player.expanded;
}

export function setExpanded(value: boolean) {
  player.expanded = value;
}

let loadGeneration = 0;
let tickUnlisten: (() => void) | null = null;
let playPending = false;
let cueHoldMs: number | null = null;
// After seeking a *playing* track, the native backend keeps emitting ticks at
// the old position for a few frames while the seek lands. Hold the playhead at
// the target and ignore those stale ticks until one arrives near it (or we time
// out), so the playhead doesn't flicker between the old and new positions.
let seekTargetMs: number | null = null;
let seekTargetAt = 0;
const SEEK_SETTLE_MS = 350;
const SEEK_TIMEOUT_MS = 800;
// Optimistic pause: UI stops immediately; ignore stale playing=true ticks until
// the backend IPC returns (same class of race as seek settle).
let pausePending = false;

function formatBpm(tempo: number): string {
  if (!tempo) return '—';
  return (tempo / 100).toFixed(2);
}

function mapCues(cuePoints: any[], durationMs: number): PlayerCue[] {
  if (!durationMs || !cuePoints?.length) return [];
  return cuePoints.map((c, i) => ({
    position: (c.time_ms ?? 0) / durationMs,
    color: CUE_COLORS[i % CUE_COLORS.length],
    timeMs: c.time_ms ?? 0,
  }));
}

function defaultCueMs(track: PlayerTrack): number {
  return track.cues[0]?.timeMs ?? 0;
}

// Fallback beat grid derived from BPM when the analyzer returns no beats.
// Starts at t=0, downbeat every 4 beats; not phase-aligned, but visible and
// editable via the expanded player's grid controls (nudge / set-downbeat).
function synthesizeBeats(bpm: number, durationMs: number): BeatMarker[] {
  if (!bpm || bpm <= 0 || !durationMs) return [];
  const beatMs = 60000 / bpm;
  if (beatMs < 50) return []; // implausible tempo — bail
  const out: BeatMarker[] = [];
  let bar = 1;
  for (let t = 0; t < durationMs; t += beatMs) {
    out.push({ time_ms: Math.round(t), bar_position: bar });
    bar = bar === 4 ? 1 : bar + 1;
  }
  return out;
}

/** Drop the playback-tick subscription (e.g. after switching mock ↔ CoreAudio). */
export function resetPlayerTransport() {
  tickUnlisten?.();
  tickUnlisten = null;
}

function previewFromAnalysis(analysis: any): number[] | null {
  const preview = analysis?.waveform_preview;
  if (!preview?.length) return null;
  return previewBytesFromPayload(analysis);
}

function applyAnalysisToTrack(base: PlayerTrack, analysis: any): PlayerTrack {
  const durationMs = analysis.duration_ms ?? base.durationMs;
  const next: PlayerTrack = {
    ...base,
    durationMs,
    totalSeconds: Math.max(0, Math.round(durationMs / 1000)),
  };
  if (analysis.bpm) next.bpm = Number(analysis.bpm).toFixed(2);
  if (analysis.key) next.key = analysis.key;
  if (isValidColorWaveform(analysis.waveform_color)) {
    next.colorData = analysis.waveform_color;
  } else {
    next.colorData = null;
  }
  next.colorDetail = isValidColorWaveform(analysis.waveform_color_detail)
    ? analysis.waveform_color_detail
    : next.colorData;
  next.colorOverview = isValidColorWaveform(analysis.waveform_overview)
    ? analysis.waveform_overview
    : next.colorData;
  const preview = previewFromAnalysis(analysis);
  if (preview) next.previewBytes = preview;
  if (analysis.beats?.length) {
    next.beats = analysis.beats.map((b: { time_ms: number; bar_position: number }) => ({
      time_ms: b.time_ms,
      bar_position: b.bar_position,
    }));
  } else {
    // No detected beats — synthesize a grid from BPM so one is always shown.
    const numBpm = Number(analysis.bpm) || Number(next.bpm) || 0;
    next.beats = synthesizeBeats(numBpm, durationMs);
  }
  next.cues = mapCues(analysis.cue_points, durationMs);
  return next;
}

export function currentTime() {
  if (!player.track) return '0:00';
  const sec = Math.floor(player.progress * player.track.totalSeconds);
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  return `${m}:${String(s).padStart(2, '0')}`;
}

export function currentTimeMs() {
  if (!player.track) return '.0';
  return `.${Math.floor(player.progress * 10) % 10}`;
}

export function totalTime() {
  if (!player.track) return '0:00';
  const sec = player.track.totalSeconds;
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  return `${m}:${String(s).padStart(2, '0')}`;
}

/** Subscribe to transport ticks (position + master levels). Safe to call repeatedly. */
export async function initPlayerTransport() {
  await ensureTickListener();
}

async function ensureTickListener() {
  if (tickUnlisten) return;
  tickUnlisten = await subscribePlaybackTick((tick) => {
    player.levelLeft = tick.level_left ?? 0;
    player.levelRight = tick.level_right ?? 0;
    if (!player.track) return;
    if (pausePending) {
      player.playing = false;
      return;
    }
    const dur = tick.duration_ms || player.track.durationMs;
    if (dur > 0) {
      if (seekTargetMs !== null) {
        const landed = Math.abs(tick.position_ms - seekTargetMs) < SEEK_SETTLE_MS;
        const expired = performance.now() - seekTargetAt > SEEK_TIMEOUT_MS;
        if (landed || expired) {
          seekTargetMs = null;
        } else {
          // Stale pre-seek position — keep showing the target.
          player.playing = tick.playing;
          return;
        }
      }
      player.progress = Math.max(0, Math.min(1, tick.position_ms / dur));
    }
    player.playing = tick.playing;
  });
}

interface BackendTrackInfo {
  id: number;
  source_path: string;
  title: string;
  artist: string;
  tempo: number;
  key: string;
  duration_secs: number;
}

/**
 * Normalize a selection input into the backend TrackInfo shape.
 * The library exposes UI rows ({ filePath, bpm: "124.0", raw: TrackInfo }),
 * but callers may also pass a raw backend track directly. Prefer `.raw`,
 * fall back to the row's own fields, and fail loud if no playable path exists.
 */
function normalizeTrackInput(input: any): BackendTrackInfo | null {
  const raw = input?.raw ?? input;
  const sourcePath = raw?.source_path ?? input?.filePath;
  if (!sourcePath) {
    console.error('loadPlayerTrack: selected track has no source path', input);
    return null;
  }
  return {
    id: raw?.id ?? input?.id,
    source_path: sourcePath,
    title: raw?.title ?? input?.title ?? '',
    artist: raw?.artist ?? input?.artist ?? '',
    tempo: raw?.tempo ?? 0,
    key: raw?.key ?? input?.key ?? '',
    duration_secs: raw?.duration_secs ?? 0,
  };
}

function buildBaseTrack(trackInfo: BackendTrackInfo, cover: string): PlayerTrack {
  return {
    id: trackInfo.id,
    sourcePath: trackInfo.source_path,
    title: trackInfo.title || 'Unknown',
    artist: trackInfo.artist || '—',
    cover,
    bpm: formatBpm(trackInfo.tempo),
    key: trackInfo.key || '—',
    totalSeconds: Math.max(0, Math.round(trackInfo.duration_secs || 0)),
    durationMs: Math.max(0, Math.round((trackInfo.duration_secs || 0) * 1000)),
    cues: [],
    beats: [],
    colorData: null,
    colorDetail: null,
    colorOverview: null,
    previewBytes: null,
  };
}

async function enrichPlayerTrack(gen: number, trackId: number, base: PlayerTrack) {
  try {
    const analysis = await getAnalysisData(trackId);
    if (gen !== loadGeneration || player.track?.id !== trackId) return;
    player.track = applyAnalysisToTrack(base, analysis);
  } catch {
    if (gen !== loadGeneration) return;
  } finally {
    if (gen === loadGeneration) {
      player.loading = false;
    }
  }
}

let playerAnalysisListenerInstalled = false;

/** Refresh the player strip when background analysis finishes for the loaded track. */
export function initPlayerAnalysisRefresh() {
  if (playerAnalysisListenerInstalled) return;
  playerAnalysisListenerInstalled = true;
  listenToAnalysisProgress((progress) => {
    const trackId = progress.track_id;
    const loaded = player.track;
    if (!trackId || !loaded || loaded.id !== trackId) return;
    invalidateAnalysisCache(trackId);
    player.loading = true;
    void enrichPlayerTrack(loadGeneration, trackId, loaded);
  });
}

/** Load track metadata + waveform from a library selection (UI row or backend track). */
export async function loadPlayerTrack(input: any) {
  const gen = ++loadGeneration;
  void playbackStop();
  player.playing = false;
  player.progress = 0;

  const trackInfo = normalizeTrackInput(input);
  if (!trackInfo) {
    player.track = null;
    player.loading = false;
    return;
  }

  const cover = input?.cover ?? input?.raw?.cover ?? '';
  const base = buildBaseTrack(trackInfo, cover);
  const cached = peekAnalysisData(trackInfo.id);
  player.track = cached ? applyAnalysisToTrack(base, cached) : base;
  player.loading = !cached;

  if (playPending && gen === loadGeneration) {
    playPending = false;
    void playFromStart();
  }

  const libTrack = library.tracks.find((t) => t.id === trackInfo.id);
  const analyzed = libTrack?.analyzed ?? trackInfo.has_analysis ?? trackInfo.tempo > 0;
  if (!analyzed) {
    requestBackgroundAnalysis(trackInfo.id);
  }

  if (cached) return;

  void enrichPlayerTrack(gen, trackInfo.id, base);
}

/** Start playback after the current selection finishes loading in the player. */
export function queuePlayAfterLoad() {
  playPending = true;
}

/** Play the loaded track from the beginning. */
export async function playFromStart() {
  if (!player.track) return;
  seekTargetMs = null;
  player.progress = 0;
  await ensureTickListener();
  await playbackPlay(player.track.sourcePath, 0, player.track.durationMs);
  player.playing = true;
}

export function clearPlayerTrack() {
  loadGeneration++;
  cueHoldMs = null;
  seekTargetMs = null;
  void playbackStop();
  player.track = null;
  player.playing = false;
  player.progress = 0;
  player.loading = false;
  player.levelLeft = 0;
  player.levelRight = 0;
  playPending = false;
}

async function pausePlayback() {
  if (!player.playing && !pausePending) return;
  pausePending = true;
  player.playing = false;
  try {
    await playbackPause();
  } catch (err) {
    console.error('playback pause failed:', err);
  } finally {
    pausePending = false;
  }
}

export async function togglePlay() {
  if (!player.track) return;
  await ensureTickListener();
  try {
    if (player.playing || pausePending) {
      await pausePlayback();
    } else {
      // Always (re)start from the current playhead. playbackPlay sets the
      // backend path + start offset, so this works even after a scrub on a
      // track that was never played (where resume would no-op on a null path).
      const ms = Math.round(player.progress * player.track.durationMs);
      await playbackPlay(player.track.sourcePath, ms, player.track.durationMs);
      player.playing = true;
    }
  } catch (err) {
    console.error('playback toggle failed:', err);
    player.playing = false;
    pausePending = false;
  }
}

/** CDJ-style CUE hold: preview from the hot cue (or start) while pressed. */
export async function cuePreviewStart() {
  if (!player.track) return;
  const dur = player.track.durationMs;
  const ms = dur > 0 ? Math.min(defaultCueMs(player.track), dur) : defaultCueMs(player.track);
  cueHoldMs = ms;
  await ensureTickListener();
  if (dur > 0) player.progress = ms / dur;
  try {
    await playbackPlay(player.track.sourcePath, ms, dur || undefined);
    player.playing = true;
  } catch (err) {
    console.error('cue preview failed:', err);
    cueHoldMs = null;
    player.playing = false;
  }
}

export async function cuePreviewStop() {
  if (!player.track || cueHoldMs === null) return;
  const ms = cueHoldMs;
  const dur = player.track.durationMs;
  cueHoldMs = null;
  try {
    await pausePlayback();
    if (dur > 0) player.progress = ms / dur;
    await playbackSetPosition(ms);
  } catch (err) {
    console.error('cue preview stop failed:', err);
  }
}

export async function play() {
  if (!player.track) return;
  await ensureTickListener();
  const ms = Math.round(player.progress * player.track.durationMs);
  await playbackPlay(player.track.sourcePath, ms, player.track.durationMs);
  player.playing = true;
}

export async function pause() {
  await pausePlayback();
}

// ── Beat-grid editing ──────────────────────────────────────────────────────
// Local, visual-only edits to the displayed grid. These shift the in-memory
// beat markers so the waveform re-renders; they are NOT yet persisted back to
// the analysis cache or the USB writer.
const BEAT_NUDGE_MS = 10;

/** Shift every beat marker by deltaMs (negative = earlier/left). */
export function beatGridShift(deltaMs: number) {
  const t = player.track;
  if (!t || !t.beats.length) return;
  t.beats = t.beats.map((b) => ({ ...b, time_ms: b.time_ms + deltaMs }));
}

export function beatGridNudgeLeft() {
  beatGridShift(-BEAT_NUDGE_MS);
}

export function beatGridNudgeRight() {
  beatGridShift(BEAT_NUDGE_MS);
}

/** Snap the grid so the nearest downbeat lands on the current playhead. */
export function beatGridSetCurrent() {
  const t = player.track;
  if (!t || !t.beats.length || !t.durationMs) return;
  const playMs = player.progress * t.durationMs;
  const downbeats = t.beats.filter((b) => b.bar_position === 1);
  const pool = downbeats.length ? downbeats : t.beats;
  let nearest = pool[0];
  let best = Infinity;
  for (const b of pool) {
    const d = Math.abs(b.time_ms - playMs);
    if (d < best) {
      best = d;
      nearest = b;
    }
  }
  beatGridShift(playMs - nearest.time_ms);
}

export async function seekToProgress(progress: number) {
  if (!player.track) return;
  const p = Math.max(0, Math.min(1, progress));
  player.progress = p;
  const ms = Math.round(p * player.track.durationMs);
  await ensureTickListener();
  if (player.playing) {
    seekTargetMs = ms;
    seekTargetAt = performance.now();
    await playbackSeek(ms);
  } else {
    // Paused/stopped: persist the position so play/resume starts from here.
    await playbackSetPosition(ms);
  }
}
