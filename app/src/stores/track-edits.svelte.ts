/**
 * track-edits.svelte.ts — hand-set BPM / key of a track. A BPM change re-spaces the beat grid.
 * Persists through the library (re-analysis keeps these) and updates the table
 * row, the inspector and the loaded player track.
 */

import { library, bumpLibraryTracks } from './library.svelte.ts';
import { flushBeatGridEdits, player, refreshLoadedBeatGrid, toggleExpanded } from './player.svelte.ts';
import { selectedTrack } from './selection.svelte.ts';
import { tick } from 'svelte';
import { rescaleBeats, type BeatMarker } from '../lib/beatGrid.ts';
import { createPersister } from '../lib/persister.ts';
import { BPM_MAX, BPM_MIN, doubleBpm, halveBpm, stepBpm } from '../lib/tempo.ts';
import type { Tier } from '../lib/tempo.ts';
import {
  getAnalysisData,
  releaseUserEdits,
  rescaleBeatGrid,
  setTrackKey,
  setTrackRating,
  updateTrackFields,
  type TrackFieldEdits,
} from '../services/tauri.svelte.ts';

function trackById(trackId: number) {
  return library.tracks.find((t) => t.id === trackId);
}

function reportFailure(what: string, err: unknown) {
  console.error(`${what} failed:`, err);
  library.statusMessage = `${what} failed: ${err}`;
}

function showBpm(trackId: number, bpm: number) {
  const track = trackById(trackId);
  if (track) {
    track.bpm = bpm.toFixed(1);
    track.bpmExact = bpm;
    track.bpmLocked = true;
    bumpLibraryTracks();
  }
  if (player.track?.id === trackId) player.track = { ...player.track, bpm: bpm.toFixed(2) };
}

let respacedGrid: { trackId: number; from: BeatMarker[]; to: BeatMarker[] } | null = null;

function respaceLoadedGrid(trackId: number, bpm: number) {
  const loaded = player.track;
  if (!loaded || loaded.id !== trackId || !loaded.beats.length) return;
  const from = respacedGrid?.trackId === trackId && respacedGrid.to === loaded.beats ? respacedGrid.from : loaded.beats;
  const durationSecs = trackById(trackId)?.raw?.duration_secs;
  const durationMs = durationSecs ? durationSecs * 1000 : loaded.durationMs;
  const to = rescaleBeats(from, bpm, durationMs);
  respacedGrid = { trackId, from, to };
  player.track = { ...loaded, beats: to };
}

const bpmPersister = createPersister<number>(saveBpmAndGrid, (err) => reportFailure('Setting the BPM', err));

function currentBpm(trackId: number): number {
  const row = trackById(trackId);
  if (row?.bpmExact) return row.bpmExact;
  return player.track?.id === trackId ? Number(player.track.bpm) : NaN;
}

async function saveBpmAndGrid(trackId: number, bpm: number): Promise<void> {
  await flushBeatGridEdits();
  try {
    await rescaleBeatGrid(trackId, bpm);
  } catch (err) {
    reportFailure('Setting the BPM', err);
    const stored = await getAnalysisData(trackId, true);
    if (stored?.bpm) showBpm(trackId, Number(stored.bpm));
    await refreshLoadedBeatGrid(trackId);
    return;
  }
  await refreshLoadedBeatGrid(trackId);
}

/** Set the track's own BPM: shown at once, the beat grid is re-spaced from its first downbeat and saved once the edits pause. */
export function commitTrackBpm(trackId: number, bpm: number): boolean {
  if (!Number.isFinite(bpm) || bpm < BPM_MIN || bpm > BPM_MAX) return false;
  showBpm(trackId, bpm);
  respaceLoadedGrid(trackId, bpm);
  bpmPersister.schedule(trackId, bpm);
  return true;
}

export function stepTrackBpm(trackId: number, units: number, tier: Tier): boolean {
  const current = currentBpm(trackId);
  return Number.isFinite(current) && commitTrackBpm(trackId, stepBpm(current, units, tier));
}

export function doubleTrackBpm(trackId: number): boolean {
  const next = doubleBpm(currentBpm(trackId));
  return next !== null && commitTrackBpm(trackId, next);
}

export function halveTrackBpm(trackId: number): boolean {
  const next = halveBpm(currentBpm(trackId));
  return next !== null && commitTrackBpm(trackId, next);
}

export const bpmEntry = $state({ request: 0 });

/** The track the BPM commands act on: the loaded one, else the selected analysed one. */
export function bpmTargetId(): number | null {
  const loaded = player.track;
  if (loaded && Number(loaded.bpm) > 0) return loaded.id;
  const selected = selectedTrack();
  return selected?.analyzed && selected.bpmExact ? selected.id : null;
}

export function loadedTrackHasBpm(): boolean {
  return Number(player.track?.bpm) > 0;
}

/** Open the big player and put its track BPM field into typing mode. */
export async function requestBpmEntry(): Promise<void> {
  if (!player.expanded) {
    toggleExpanded();
    await tick();
  }
  bpmEntry.request++;
}

export async function commitTrackKey(trackId: number, key: string): Promise<boolean> {
  try {
    await setTrackKey(trackId, key);
  } catch (err) {
    reportFailure('Setting the key', err);
    return false;
  }
  const track = trackById(trackId);
  if (track) {
    track.key = key;
    track.keyLocked = true;
    bumpLibraryTracks();
  }
  if (player.track?.id === trackId) player.track = { ...player.track, key };
  return true;
}

export async function commitTrackRating(trackId: number, rating: number): Promise<boolean> {
  try {
    await setTrackRating(trackId, rating);
  } catch (err) {
    reportFailure('Setting the rating', err);
    return false;
  }
  const track = trackById(trackId);
  if (track) {
    track.rating = rating;
    bumpLibraryTracks();
  }
  return true;
}

export async function releaseTrackEdits(trackId: number): Promise<void> {
  try {
    await releaseUserEdits(trackId);
  } catch (err) {
    reportFailure('Releasing the hand-set values', err);
    return;
  }
  const track = trackById(trackId);
  if (track) {
    track.bpmLocked = false;
    track.keyLocked = false;
    bumpLibraryTracks();
  }
}

export async function commitTrackFields(trackId: number, edits: TrackFieldEdits): Promise<boolean> {
  let saved;
  try {
    saved = await updateTrackFields(trackId, edits);
  } catch (err) {
    reportFailure('Saving the tags', err);
    return false;
  }
  const track = trackById(trackId);
  if (track) {
    track.title = saved.title;
    track.artist = saved.artist || '—';
    track.album = saved.album || '—';
    track.genre = saved.genre || '—';
    if (track.raw) track.raw = { ...track.raw, ...saved };
    bumpLibraryTracks();
  }
  if (player.track?.id === trackId) {
    player.track = { ...player.track, title: saved.title, artist: saved.artist || '—' };
  }
  return true;
}
