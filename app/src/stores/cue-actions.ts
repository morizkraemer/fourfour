/**
 * cue-actions.ts — What a DJ does with cues: set one at the playhead, jump to one, rename, recolour, delete.
 * Setting needs the playhead, so it only works on the loaded track; the other edits work on any track.
 */

import { player, currentPositionMs, seekToMs, seekToMsAndPlay } from './player.svelte.ts';
import { cuesOf, editCues } from './cues.svelte.ts';
import { quantize } from './quantize.svelte.ts';
import { quantizedMs } from '../lib/quantize.ts';
import {
  hotCueIn,
  withCueColor,
  withCuePad,
  withCueMoved,
  withCueName,
  withHotCue,
  withMemoryCue,
  withoutCue,
  type Cue,
} from '../lib/cues.ts';

/** Set hot cue `slot` (or the first free slot) at the playhead; false when the slot or every slot is taken. */
export function addHotCueAtPlayhead(slot: number | null = null): boolean {
  const track = player.track;
  if (!track) return false;
  let added = false;
  editCues(track.id, (cues) => {
    const result = withHotCue(cues, quantizedMs(track.beats ?? [], currentPositionMs(), quantize.on), slot);
    added = result !== null;
    return result ? result.cues : cues;
  });
  return added;
}

/** Moves a hot cue to `timeMs` (the caller snaps it to the grid when it should). */
export function moveHotCue(trackId: number, key: string, timeMs: number): void {
  editCues(trackId, (cues) => withCueMoved(cues, key, timeMs));
}

export function addMemoryCueAtPlayhead(): void {
  const track = player.track;
  if (!track) return;
  editCues(track.id, (cues) => withMemoryCue(cues, currentPositionMs()).cues);
}

/** A hot cue pad: an empty slot is set at the playhead, a set one jumps there and plays. */
export async function triggerHotCue(slot: number): Promise<void> {
  const track = player.track;
  if (!track) return;
  const cue = hotCueIn(cuesOf(track.id), slot);
  if (!cue) {
    addHotCueAtPlayhead(slot);
    return;
  }
  await seekToMsAndPlay(cue.timeMs);
}

export async function jumpToHotCue(slot: number): Promise<void> {
  const track = player.track;
  const cue = track && hotCueIn(cuesOf(track.id), slot);
  if (cue) await seekToMs(cue.timeMs);
}

export function clearAllCues(): void {
  const track = player.track;
  if (track) editCues(track.id, (cues) => (cues.length ? [] : cues));
}

export function jumpToCue(cue: Pick<Cue, 'timeMs'>): Promise<void> {
  return seekToMs(cue.timeMs);
}

export function deleteHotCue(slot: number): void {
  const track = player.track;
  if (!track) return;
  const cue = hotCueIn(cuesOf(track.id), slot);
  if (cue) deleteCue(track.id, `h${slot}`);
}

export function deleteCue(trackId: number, key: string): void {
  editCues(trackId, (cues) => withoutCue(cues, key));
}

export function renameCue(trackId: number, key: string, name: string): void {
  editCues(trackId, (cues) => withCueName(cues, key, name));
}

export function recolorCue(trackId: number, key: string, color: string): void {
  editCues(trackId, (cues) => withCueColor(cues, key, color));
}

export function setCuePad(trackId: number, key: string, slot: number | null): void {
  editCues(trackId, (cues) => withCuePad(cues, key, slot) ?? cues);
}
