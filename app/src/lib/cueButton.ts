export interface CueButtonState {
  cuePointMs: number;
  previewing: boolean;
}

export interface CueTransport {
  playing: boolean;
  positionMs: number;
}

export type CueCommand =
  | { kind: 'play-from'; ms: number }
  | { kind: 'return'; ms: number }
  | { kind: 'none' };

export interface CueStep {
  state: CueButtonState;
  command: CueCommand;
}

const none: CueCommand = { kind: 'none' };

export function initialCueButton(cuePointMs = 0): CueButtonState {
  return { cuePointMs, previewing: false };
}

/**
 * CDJ CUE pressed. Playing: jump back to the cue point and pause. Paused: the playhead becomes the
 * cue point and the track plays from it for as long as the button is held.
 */
export function cuePress(state: CueButtonState, transport: CueTransport): CueStep {
  if (transport.playing) {
    return { state: { ...state, previewing: false }, command: { kind: 'return', ms: state.cuePointMs } };
  }
  if (state.previewing) return { state, command: none };
  const cuePointMs = Math.max(0, Math.round(transport.positionMs));
  return { state: { cuePointMs, previewing: true }, command: { kind: 'play-from', ms: cuePointMs } };
}

/** CUE released: a preview ends at the cue point. */
export function cueRelease(state: CueButtonState): CueStep {
  if (!state.previewing) return { state, command: none };
  return { state: { ...state, previewing: false }, command: { kind: 'return', ms: state.cuePointMs } };
}

/** CUE tapped without a hold (keyboard): playing returns to the cue point, paused sets it at the playhead. */
export function cueTap(state: CueButtonState, transport: CueTransport): CueStep {
  if (transport.playing) {
    return { state: { ...state, previewing: false }, command: { kind: 'return', ms: state.cuePointMs } };
  }
  return { state: { cuePointMs: Math.max(0, Math.round(transport.positionMs)), previewing: false }, command: none };
}

/** Play pressed while CUE is held: the track keeps playing when CUE is released. */
export function playWhileCueHeld(state: CueButtonState): CueButtonState {
  return state.previewing ? { ...state, previewing: false } : state;
}
