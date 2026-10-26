import { describe, expect, it } from 'vitest';
import { cuePress, cueRelease, cueTap, initialCueButton, playWhileCueHeld } from './cueButton.ts';

const paused = (positionMs: number) => ({ playing: false, positionMs });
const playing = (positionMs: number) => ({ playing: true, positionMs });

describe('CUE pressed while paused', () => {
  it('sets the cue point at the playhead and plays from it', () => {
    const step = cuePress(initialCueButton(0), paused(12_345.6));
    expect(step.state).toEqual({ cuePointMs: 12_346, previewing: true });
    expect(step.command).toEqual({ kind: 'play-from', ms: 12_346 });
  });

  it('moves the cue point when the playhead was moved away from it', () => {
    const step = cuePress(initialCueButton(1000), paused(8000));
    expect(step.state.cuePointMs).toBe(8000);
  });

  it('plays from the same spot when the playhead is on the cue point', () => {
    const step = cuePress(initialCueButton(5000), paused(5000));
    expect(step.command).toEqual({ kind: 'play-from', ms: 5000 });
  });

  it('never sets a negative cue point', () => {
    expect(cuePress(initialCueButton(), paused(-20)).state.cuePointMs).toBe(0);
  });

  it('ignores a second press while the preview runs', () => {
    const first = cuePress(initialCueButton(), paused(4000));
    const second = cuePress(first.state, paused(4200));
    expect(second.command).toEqual({ kind: 'none' });
    expect(second.state).toBe(first.state);
  });
});

describe('CUE released', () => {
  it('ends a preview at the cue point', () => {
    const held = cuePress(initialCueButton(), paused(4000)).state;
    const step = cueRelease(held);
    expect(step.command).toEqual({ kind: 'return', ms: 4000 });
    expect(step.state).toEqual({ cuePointMs: 4000, previewing: false });
  });

  it('does nothing when no preview is running', () => {
    const idle = initialCueButton(900);
    expect(cueRelease(idle)).toEqual({ state: idle, command: { kind: 'none' } });
  });
});

describe('CUE pressed while playing', () => {
  it('returns to the cue point and pauses, keeping the cue point', () => {
    const step = cuePress(initialCueButton(2500), playing(60_000));
    expect(step.command).toEqual({ kind: 'return', ms: 2500 });
    expect(step.state).toEqual({ cuePointMs: 2500, previewing: false });
  });

  it('then does not preview on release', () => {
    const step = cuePress(initialCueButton(2500), playing(60_000));
    expect(cueRelease(step.state).command).toEqual({ kind: 'none' });
  });
});

describe('play pressed while CUE is held', () => {
  it('keeps the track playing when CUE is released', () => {
    const held = cuePress(initialCueButton(), paused(4000)).state;
    const locked = playWhileCueHeld(held);
    expect(locked).toEqual({ cuePointMs: 4000, previewing: false });
    expect(cueRelease(locked).command).toEqual({ kind: 'none' });
  });

  it('changes nothing without a preview', () => {
    const idle = initialCueButton(3);
    expect(playWhileCueHeld(idle)).toBe(idle);
  });
});

describe('a full set-and-return sequence', () => {
  it('sets, previews, returns, plays on, and comes back', () => {
    let state = initialCueButton(0);

    let step = cuePress(state, paused(30_000));
    state = step.state;
    expect(step.command).toEqual({ kind: 'play-from', ms: 30_000 });
    step = cueRelease(state);
    state = step.state;
    expect(step.command).toEqual({ kind: 'return', ms: 30_000 });

    step = cuePress(state, playing(90_000));
    expect(step.command).toEqual({ kind: 'return', ms: 30_000 });
  });
});

describe('CUE tapped (keyboard)', () => {
  it('returns to the cue point while playing', () => {
    const step = cueTap(initialCueButton(1500), playing(70_000));
    expect(step.command).toEqual({ kind: 'return', ms: 1500 });
  });

  it('only sets the cue point while paused', () => {
    const step = cueTap(initialCueButton(1500), paused(44_100));
    expect(step.command).toEqual({ kind: 'none' });
    expect(step.state).toEqual({ cuePointMs: 44_100, previewing: false });
  });
});
