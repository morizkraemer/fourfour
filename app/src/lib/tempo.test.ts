import { describe, expect, it } from 'vitest';
import {
  clampPercent,
  clampShift,
  formatPercent,
  formatShift,
  keyShiftScrub,
  parsePercent,
  percentForBpm,
  percentFromRatio,
  playingBpm,
  playingBpmScrub,
  ratioFromPercent,
  shiftForKey,
  shiftedKey,
  smallestRangeFor,
  stepPercent,
  tempoPercentScrub,
} from './tempo.ts';

describe('percent and ratio', () => {
  it('maps percent to the playback ratio and back', () => {
    expect(ratioFromPercent(0)).toBe(1);
    expect(ratioFromPercent(8)).toBeCloseTo(1.08, 12);
    expect(ratioFromPercent(-16)).toBeCloseTo(0.84, 12);
    expect(percentFromRatio(1.08)).toBe(8);
    expect(percentFromRatio(1)).toBe(0);
  });

  it('clamps to a range and rejects garbage', () => {
    expect(clampPercent(12, 8)).toBe(8);
    expect(clampPercent(-12, 8)).toBe(-8);
    expect(clampPercent(77)).toBe(50);
    expect(clampPercent(Number.NaN)).toBe(0);
  });

  it('finds the smallest range that holds a typed value', () => {
    expect(smallestRangeFor(3)).toBe(8);
    expect(smallestRangeFor(-8)).toBe(8);
    expect(smallestRangeFor(8.01)).toBe(16);
    expect(smallestRangeFor(-30)).toBe(50);
  });
});

describe('playing BPM', () => {
  it('is the analysed BPM times the ratio', () => {
    expect(playingBpm(128, 0)).toBe(128);
    expect(playingBpm(128, 8)).toBeCloseTo(138.24, 9);
    expect(playingBpm(125, -4)).toBeCloseTo(120, 9);
  });

  it('typing a BPM sets the tempo that reaches it', () => {
    expect(percentForBpm(138.24, 128)).toBe(8);
    expect(percentForBpm(128, 128)).toBe(0);
    expect(percentForBpm(120, 125)).toBe(-4);
    expect(percentForBpm(1000, 128)).toBe(50);
  });
});

describe('percent steps', () => {
  it('uses CDJ-like fine steps per range', () => {
    expect(stepPercent(0, 1, 'normal', 8)).toBe(0.02);
    expect(stepPercent(0, -1, 'normal', 8)).toBe(-0.02);
    expect(stepPercent(0, 1, 'normal', 16)).toBe(0.05);
    expect(stepPercent(0, 1, 'normal', 50)).toBe(0.1);
    expect(stepPercent(0, 1, 'fine', 8)).toBe(0.01);
    expect(stepPercent(0, 1, 'coarse', 8)).toBe(0.1);
  });

  it('snaps to the grid and does not accumulate float error', () => {
    expect(stepPercent(1.234, 1, 'normal', 8)).toBe(1.24);
    expect(stepPercent(1.234, -1, 'normal', 8)).toBe(1.22);
    let value = 0;
    for (let i = 0; i < 100; i++) value = stepPercent(value, 1, 'normal', 8);
    expect(value).toBe(2);
  });

  it('stops at the end of the range', () => {
    expect(stepPercent(7.99, 5, 'coarse', 8)).toBe(8);
    expect(stepPercent(-7.99, -5, 'coarse', 8)).toBe(-8);
  });
});

describe('percent text', () => {
  it('formats signed with two decimals', () => {
    expect(formatPercent(0)).toBe('0.00%');
    expect(formatPercent(4.2)).toBe('+4.20%');
    expect(formatPercent(-3.456)).toBe('−3.46%');
    expect(formatPercent(0.001)).toBe('0.00%');
  });

  it('parses what people type', () => {
    expect(parsePercent('4.2')).toBe(4.2);
    expect(parsePercent('+4,2 %')).toBe(4.2);
    expect(parsePercent('-3.5%')).toBe(-3.5);
    expect(parsePercent('−3.5')).toBe(-3.5);
    expect(parsePercent('51')).toBeNull();
    expect(parsePercent('abc')).toBeNull();
  });
});

describe('key shift', () => {
  it('moves a Camelot key by semitones (+7 wheel steps each)', () => {
    expect(shiftedKey('8A', 0)).toBe('8A');
    expect(shiftedKey('8A', 1)).toBe('3A');
    expect(shiftedKey('8A', -1)).toBe('1A');
    expect(shiftedKey('8A', 12)).toBe('8A');
    expect(shiftedKey('5B', 2)).toBe('7B');
    expect(shiftedKey('5B', -2)).toBe('3B');
  });

  it('follows the notation the analysed key is written in', () => {
    expect(shiftedKey('Am', 2)).toBe('Bm');
    expect(shiftedKey('Am', 3)).toBe('Cm');
    expect(shiftedKey('C', 1)).toBe('C#');
    expect(shiftedKey('Bb', 1)).toBe('B');
    expect(shiftedKey('Bbm', 2)).toBe('Cm');
    expect(shiftedKey('Eb', 1)).toBe('E');
    expect(shiftedKey('Ab', 1)).toBe('A');
  });

  it('has no key to show for an unanalysed track', () => {
    expect(shiftedKey('—', 2)).toBeNull();
    expect(shiftedKey('', 2)).toBeNull();
  });

  it('finds the shift that reaches a typed key, nearest to the current one', () => {
    expect(shiftForKey('8A', '3A')).toBe(1);
    expect(shiftForKey('8A', 'Bm')).toBe(2);
    expect(shiftForKey('8A', '8A')).toBe(0);
    expect(shiftForKey('8A', '2A')).toBe(-6);
    expect(shiftForKey('8A', '2A', 7)).toBe(6);
    expect(shiftForKey('8A', '3A')).toBe(1);
    expect(shiftForKey('8A', '3A', 11)).toBe(1);
    expect(shiftForKey('8A', '9B')).toBeNull();
    expect(shiftForKey('8A', 'garbage')).toBeNull();
  });

  it('clamps to a full octave', () => {
    expect(clampShift(13)).toBe(12);
    expect(clampShift(-40)).toBe(-12);
    expect(clampShift(2.6)).toBe(3);
    expect(formatShift(2)).toBe('+2 st');
    expect(formatShift(-3)).toBe('−3 st');
    expect(formatShift(0)).toBe('0 st');
  });
});

describe('scrub models', () => {
  it('the BPM model steps within the range and types within Wide', () => {
    const model = playingBpmScrub(128, 8);
    expect(model.step(128, 1)).toBe(128.1);
    expect(model.step(138.2, 5, 'coarse')).toBe(138.24);
    expect(model.step(117.8, -5, 'coarse')).toBe(117.76);
    expect(model.parse('140')).toBe(140);
    expect(model.parse('200')).toBeNull();
    expect(model.parse('x')).toBeNull();
    expect(model.valid(0)).toBe(false);
  });

  it('the tempo model steps by range and resets with the action', () => {
    const model = tempoPercentScrub(16);
    expect(model.step(0, 1, 'normal')).toBe(0.05);
    expect(model.format(model.step(0, 1, 'normal'))).toBe('+0.05%');
    expect(model.actions[0].apply()).toBe(0);
  });

  it('the key model shows the resulting key and steps the shift', () => {
    const model = keyShiftScrub('8A', 0);
    expect(model.format(0)).toBe('8A');
    expect(model.format(2)).toBe('10A');
    expect(model.step(0, 1)).toBe(1);
    expect(model.step(12, 1)).toBe(12);
    expect(model.step(-12, -1)).toBe(-12);
    expect(model.parse('10A')).toBe(2);
    expect(model.parse('9B')).toBeNull();
    expect(keyShiftScrub('—', 0).valid(0)).toBe(false);
  });
});
