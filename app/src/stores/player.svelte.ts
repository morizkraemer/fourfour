/**
 * player.svelte.ts — Playback state for the compact player bar.
 */

import { getAnalysisData } from '../services/tauri.svelte.ts';
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
  colorData: ColorWaveSample[] | null;
  previewBytes: number[] | null;
}

const CUE_COLORS = ['#4ade80', '#60a5fa', '#f472b6', '#fbbf24', '#a78bfa', '#fb7185', '#34d399', '#38bdf8'];

export const player = $state({
  playing: false,
  progress: 0.0,
  track: null as PlayerTrack | null,
  loading: false,
});

let loadGeneration = 0;
let tickUnlisten: (() => void) | null = null;

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

async function ensureTickListener() {
  if (tickUnlisten) return;
  tickUnlisten = await subscribePlaybackTick((tick) => {
    if (!player.track) return;
    const dur = tick.duration_ms || player.track.durationMs;
    if (dur > 0) {
      player.progress = Math.max(0, Math.min(1, tick.position_ms / dur));
    }
    player.playing = tick.playing;
  });
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
  await playbackStop();
  player.playing = false;
  player.progress = 0;
  player.loading = true;

  const base: PlayerTrack = {
    id: trackInfo.id,
    sourcePath: trackInfo.source_path,
    title: trackInfo.title || 'Unknown',
    artist: trackInfo.artist || '—',
    bpm: formatBpm(trackInfo.tempo),
    key: trackInfo.key || '—',
    totalSeconds: Math.max(0, Math.round(trackInfo.duration_secs || 0)),
    durationMs: Math.max(0, Math.round((trackInfo.duration_secs || 0) * 1000)),
    cues: [],
    colorData: null,
    previewBytes: null,
  };
  player.track = base;

  try {
    const analysis = await getAnalysisData(trackInfo.id);
    if (gen !== loadGeneration) return;

    const durationMs = analysis.duration_ms ?? base.durationMs;
    base.durationMs = durationMs;
    base.totalSeconds = Math.max(0, Math.round(durationMs / 1000));
    if (analysis.bpm) base.bpm = Number(analysis.bpm).toFixed(2);
    if (analysis.key) base.key = analysis.key;
    if (analysis.waveform_color?.length) {
      base.colorData = analysis.waveform_color;
    }
    if (analysis.waveform_preview?.length) {
      base.previewBytes = Array.from(analysis.waveform_preview);
    }
    base.cues = mapCues(analysis.cue_points, durationMs);
    player.track = { ...base };
  } catch {
    if (gen !== loadGeneration) return;
    // Keep metadata; waveform stays empty until analyzed.
  } finally {
    if (gen === loadGeneration) player.loading = false;
  }
}

export function clearPlayerTrack() {
  loadGeneration++;
  void playbackStop();
  player.track = null;
  player.playing = false;
  player.progress = 0;
  player.loading = false;
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
    await playbackPlay(player.track.sourcePath, 0);
    player.playing = true;
  }
}

export async function play() {
  if (!player.track) return;
  await ensureTickListener();
  await playbackPlay(player.track.sourcePath, Math.round(player.progress * player.track.durationMs));
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
