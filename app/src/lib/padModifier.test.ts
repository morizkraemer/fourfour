import { describe, expect, it } from 'vitest';
import { padAction, padFace } from './padModifier.ts';

describe('hot cue pad with Option held', () => {
  it('deletes a set pad and leaves empty pads alone', () => {
    expect(padAction(true, true)).toBe('delete');
    expect(padAction(true, false)).toBe('set');
    expect(padAction(false, true)).toBe('jump');
  });
  it('shows × only on set pads while held', () => {
    expect(padFace('C', true, true)).toBe('×');
    expect(padFace('C', false, true)).toBe('C');
    expect(padFace('C', true, false)).toBe('C');
  });
});
