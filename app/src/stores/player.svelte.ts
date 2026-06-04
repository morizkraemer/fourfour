/**
 * player.svelte.ts — Playback state for the compact player bar.
 */

import {
  getAnalysisData,
  peekAnalysisData,
} from '../services/tauri.svelte.ts';
import {
  playbackPause,
  playbackPlay,
  playbackResume,
  playbackSeek,
  playbackStop,
  subscribePlaybackTick,
} from '../services/playback.svelte.ts';

export interface PlayerCue {
  position: number;
  color: string;
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
  bpm: string;
  key: string;
  totalSeconds: number;
  durationMs: number;
  cues: PlayerCue[];
  beats: BeatMarker[];
  colorData: ColorWaveSample[] | null;
  previewBytes: number[] | null;
}

const CUE_COLORS = ['#4ade80', '#60a5fa', '#f472b6', '#fbbf24', '#a78bfa', '#fb7185', '#34d399', '#38bdf8'];

export const player = $state({
  playing: false,
  progress: 0.0,
  track: null as PlayerTrack | null,
  loading: false,
  levelLeft: 0,
  levelRight: 0,
});

let loadGeneration = 0;
let tickUnlisten: (() => void) | null = null;
let playPending = false;

function formatBpm(tempo: number): string {
  if (!tempo) return '—';
  return (tempo / 100).toFixed(2);
}

function mapCues(cuePoints: any[], durationMs: number): PlayerCue[] {
  if (!durationMs || !cuePoints?.length) return [];
  return cuePoints.map((c, i) => ({
    position: (c.time_ms ?? 0) / durationMs,
    color: CUE_COLORS[i % CUE_COLORS.length],
  }));
}

function previewFromAnalysis(analysis: any): number[] | null {
  const preview = analysis?.waveform_preview;
  if (!preview?.length) return null;
  return Array.isArray(preview) ? preview : Array.from(preview);
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
  if (analysis.waveform_color?.length) {
    next.colorData = analysis.waveform_color;
  }
  const preview = previewFromAnalysis(analysis);
  if (preview) next.previewBytes = preview;
  if (analysis.beats?.length) {
    next.beats = analysis.beats.map((b: { time_ms: number; bar_position: number }) => ({
      time_ms: b.time_ms,
      bar_position: b.bar_position,
    }));
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
    const dur = tick.duration_ms || player.track.durationMs;
    if (dur > 0) {
      player.progress = Math.max(0, Math.min(1, tick.position_ms / dur));
    }
    player.playing = tick.playing;
  });
}

function buildBaseTrack(trackInfo: {
  id: number;
  source_path: string;
  title: string;
  artist: string;
  tempo: number;
  key: string;
  duration_secs: number;
}): PlayerTrack {
  return {
    id: trackInfo.id,
    sourcePath: trackInfo.source_path,
    title: trackInfo.title || 'Unknown',
    artist: trackInfo.artist || '—',
    bpm: formatBpm(trackInfo.tempo),
    key: trackInfo.key || '—',
    totalSeconds: Math.max(0, Math.round(trackInfo.duration_secs || 0)),
    durationMs: Math.max(0, Math.round((trackInfo.duration_secs || 0) * 1000)),
    cues: [],
    beats: [],
    colorData: null,
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

/** Load track metadata + waveform from library selection. */
export async function loadPlayerTrack(trackInfo: {
  id: number;
  source_path: string;
  title: string;
  artist: string;
  tempo: number;
  key: string;
  duration_secs: number;
}) {
  const gen = ++loadGeneration;
  void playbackStop();
  player.playing = false;
  player.progress = 0;

  const base = buildBaseTrack(trackInfo);
  const cached = peekAnalysisData(trackInfo.id);
  player.track = cached ? applyAnalysisToTrack(base, cached) : base;
  player.loading = !cached;

  if (playPending && gen === loadGeneration) {
    playPending = false;
    void playFromStart();
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
  player.progress = 0;
  await ensureTickListener();
  await playbackPlay(player.track.sourcePath, 0, player.track.durationMs);
  player.playing = true;
}

export function clearPlayerTrack() {
  loadGeneration++;
  void playbackStop();
  player.track = null;
  player.playing = false;
  player.progress = 0;
  player.loading = false;
  player.levelLeft = 0;
  player.levelRight = 0;
  playPending = false;
}

export async function togglePlay() {
  if (!player.track) return;
  await ensureTickListener();
  if (player.playing) {
    await playbackPause();
    player.playing = false;
  } else if (player.progress > 0 && player.progress < 1) {
    await playbackResume();
    player.playing = true;
  } else {
    await playbackPlay(player.track.sourcePath, 0, player.track.durationMs);
    player.playing = true;
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
  await playbackPause();
  player.playing = false;
}

export async function seekToProgress(progress: number) {
  if (!player.track) return;
  const p = Math.max(0, Math.min(1, progress));
  player.progress = p;
  const ms = Math.round(p * player.track.durationMs);
  await ensureTickListener();
  if (player.playing) {
    await playbackSeek(ms);
  }
}
