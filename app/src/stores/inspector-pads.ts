import { player } from './player.svelte.ts';
import { cuesOf } from './cues.svelte.ts';
import { addHotCueAtPlayhead, deleteCue, fireMemoryCue, triggerHotCue } from './cue-actions.ts';
import { seekTrackToMs } from './inspector-seek.ts';
import { hotCueIn } from '../lib/cues.ts';
import { padAction } from '../lib/padModifier.ts';

/** A cue pad in the inspector: an empty one saves a cue at the playhead (loaded track only), a set one jumps there and plays like the player's pads (only jumps on another track), Alt deletes it. */
export async function pressInspectorPad(track: { id: number }, slot: number, altHeld: boolean): Promise<void> {
  const cue = hotCueIn(cuesOf(track.id), slot);
  const action = padAction(altHeld, !!cue);
  if (action === 'set') {
    if (player.track?.id === track.id) addHotCueAtPlayhead(slot);
  } else if (cue && action === 'delete') {
    deleteCue(track.id, `h${slot}`);
  } else if (cue && player.track?.id === track.id) {
    await triggerHotCue(slot);
  } else if (cue) {
    await seekTrackToMs(track, cue.timeMs);
  }
}

/** A cue chip's "jump" in the inspector: a memory cue of the loaded track fires like a pad (jump and play); anything else only jumps (loading the track first). */
export async function jumpToInspectorCue(track: { id: number }, chip: { timeMs: number; padded: boolean }): Promise<void> {
  if (!chip.padded && player.track?.id === track.id) await fireMemoryCue(chip);
  else await seekTrackToMs(track, chip.timeMs);
}
