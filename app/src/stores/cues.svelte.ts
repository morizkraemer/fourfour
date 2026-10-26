/**
 * cues.svelte.ts — Cues of every track the app has looked at, shared by the players and the inspector.
 * An edit shows at once and is saved after a short pause (like the beat grid); while a track has an
 * unsaved edit, an analysis payload does not overwrite it.
 */

import { setCues } from '../services/tauri.svelte.ts';
import { createPersister } from '../lib/persister.ts';
import { cuesFromPayload, cuesToPayload, sameCues, type Cue, type CuePointPayload } from '../lib/cues.ts';

let byTrack = $state.raw<Record<number, Cue[]>>({});
const unsaved = new Set<number>();
let onSaveError: (message: string) => void = (message) => console.error(message);

const persister = createPersister<Cue[]>(
  async (trackId, cues) => {
    await setCues(trackId, cuesToPayload(cues));
    if (byTrack[trackId] === cues) unsaved.delete(trackId);
  },
  (err) => onSaveError(`Could not save the cues: ${err}`),
);

const NO_CUES: Cue[] = [];

export function cuesOf(trackId: number | null | undefined): Cue[] {
  return trackId == null ? NO_CUES : (byTrack[trackId] ?? NO_CUES);
}

/** Take the cues of an analysis payload, unless the user has an edit of this track that is not stored yet. */
export function loadCuesFromPayload(trackId: number, points: CuePointPayload[] | undefined) {
  if (unsaved.has(trackId)) return;
  const next = cuesFromPayload(points);
  if (byTrack[trackId] && sameCues(byTrack[trackId], next)) return;
  byTrack = { ...byTrack, [trackId]: next };
}

/** Replace the cues of a track with `edit(current)`; no change when it returns the same list. */
export function editCues(trackId: number, edit: (cues: Cue[]) => Cue[]) {
  const current = cuesOf(trackId);
  const next = edit(current);
  if (next === current) return;
  byTrack = { ...byTrack, [trackId]: next };
  unsaved.add(trackId);
  persister.schedule(trackId, next);
}

/** Write a pending cue edit now (before another track loads). */
export function flushCueEdits(): Promise<void> {
  return persister.flush();
}

export function onCueSaveError(handler: (message: string) => void) {
  onSaveError = handler;
}
