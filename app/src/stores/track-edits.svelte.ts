/**
 * track-edits.svelte.ts — hand-set BPM / key of a track.
 * Persists through the library (re-analysis keeps these) and updates the table
 * row, the inspector and the loaded player track.
 */

import { library, bumpLibraryTracks } from './library.svelte.ts';
import { player } from './player.svelte.ts';
import {
  getAnalysisData,
  releaseUserEdits,
  rescaleBeatGrid,
  setTrackBpm,
  setTrackKey,
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

export async function commitTrackBpm(trackId: number, bpm: number): Promise<boolean> {
  try {
    await setTrackBpm(trackId, bpm);
  } catch (err) {
    reportFailure('Setting the BPM', err);
    return false;
  }
  showBpm(trackId, bpm);
  return true;
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

/** Explicit second step: fit the beat grid to the track's BPM. Never done by a BPM change. */
export async function fitGridToBpm(trackId: number, bpm: number): Promise<boolean> {
  try {
    await rescaleBeatGrid(trackId, bpm);
  } catch (err) {
    reportFailure('Adjusting the beat grid', err);
    return false;
  }
  showBpm(trackId, bpm);
  if (player.track?.id === trackId) {
    const fresh = await getAnalysisData(trackId, true);
    const beats = fresh?.beats?.map((b: { time_ms: number; bar_position: number }) => ({
      time_ms: b.time_ms,
      bar_position: b.bar_position,
    }));
    if (beats?.length && player.track?.id === trackId) player.track = { ...player.track, beats };
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
