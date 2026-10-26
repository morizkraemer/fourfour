import { describe, expect, it } from 'vitest';
import {
  BPM_MAX,
  BPM_MIN,
  bpmScrub,
  doubleBpm,
  halveBpm,
  keyModifier,
  keyPosition,
  keyScrub,
  parseBpm,
  parseCamelot,
  parseKey,
  stepBpm,
  stepKey,
  tierOf,
} from '../../../prototyping/viewport/design-system/utils/scrub.js';

describe('stepBpm', () => {
  it('steps by 0.1 by default and snaps to the grid', () => {
    expect(stepBpm(128, 1)).toBe(128.1);
    expect(stepBpm(128, -1)).toBe(127.9);
    expect(stepBpm(128.37, 1)).toBe(128.4);
    expect(stepBpm(128.37, -1)).toBe(128.3);
  });

  it('steps by 0.01 when fine and by 1 when coarse', () => {
    expect(stepBpm(128, 1, 'fine')).toBe(128.01);
    expect(stepBpm(128.5, 1, 'coarse')).toBe(129);
    expect(stepBpm(128.5, -1, 'coarse')).toBe(128);
    expect(stepBpm(128, 5, 'coarse')).toBe(133);
  });

  it('does not accumulate float error', () => {
    let value = 120;
    for (let i = 0; i < 30; i++) value = stepBpm(value, 1);
    expect(value).toBe(123);
  });

  it('clamps to 40 and 250', () => {
    expect(stepBpm(249.95, 10, 'coarse')).toBe(BPM_MAX);
    expect(stepBpm(40.05, -10, 'coarse')).toBe(BPM_MIN);
  });
});

describe('tierOf', () => {
  it('maps modifiers to tiers, shift wins', () => {
    expect(tierOf({ shiftKey: false, altKey: false })).toBe('normal');
    expect(tierOf({ shiftKey: true, altKey: false })).toBe('coarse');
    expect(tierOf({ shiftKey: false, altKey: true })).toBe('fine');
    expect(tierOf({ shiftKey: true, altKey: true })).toBe('coarse');
  });
});

describe('double and halve', () => {
  it('doubles and halves within range', () => {
    expect(doubleBpm(70)).toBe(140);
    expect(halveBpm(174)).toBe(87);
    expect(halveBpm(87.5)).toBe(43.75);
  });

  it('refuses to leave the range', () => {
    expect(doubleBpm(130)).toBeNull();
    expect(halveBpm(70)).toBeNull();
  });
});

describe('parseBpm', () => {
  it('parses plain, decimal and comma input', () => {
    expect(parseBpm('128')).toBe(128);
    expect(parseBpm(' 128.5 ')).toBe(128.5);
    expect(parseBpm('127,25')).toBe(127.25);
    expect(parseBpm('127.456')).toBe(127.46);
  });

  it('rejects junk and out-of-range values', () => {
    expect(parseBpm('')).toBeNull();
    expect(parseBpm('abc')).toBeNull();
    expect(parseBpm('-120')).toBeNull();
    expect(parseBpm('39.9')).toBeNull();
    expect(parseBpm('251')).toBeNull();
    expect(parseBpm('1e2')).toBeNull();
  });
});

describe('Camelot stepping', () => {
  it('moves around the wheel and wraps', () => {
    expect(stepKey('8A', 1)).toBe('9A');
    expect(stepKey('8A', -1)).toBe('7A');
    expect(stepKey('12B', 1)).toBe('1B');
    expect(stepKey('1A', -1)).toBe('12A');
    expect(stepKey('3A', 25)).toBe('4A');
  });

  it('toggles A/B on odd steps with the toggle modifier', () => {
    expect(stepKey('8A', 1, 'toggle')).toBe('8B');
    expect(stepKey('8B', -1, 'toggle')).toBe('8A');
    expect(stepKey('8A', 2, 'toggle')).toBe('8A');
  });

  it('jumps a semitone (7 wheel steps) when coarse', () => {
    expect(stepKey('8A', 1, 'coarse')).toBe('3A');
    expect(stepKey('8A', -1, 'coarse')).toBe('1A');
  });

  it('starts from 1A when the key is unknown', () => {
    expect(stepKey('—', 1)).toBe('2A');
  });

  it('picks the key modifier', () => {
    expect(keyModifier({ altKey: true, shiftKey: false })).toBe('toggle');
    expect(keyModifier({ altKey: false, shiftKey: true })).toBe('coarse');
    expect(keyModifier({ altKey: false, shiftKey: false })).toBe('normal');
  });

  it('maps keys to a 1 to 24 aria position', () => {
    expect(keyPosition('1A')).toBe(1);
    expect(keyPosition('1B')).toBe(2);
    expect(keyPosition('12B')).toBe(24);
    expect(keyPosition('—')).toBe(0);
  });
});

describe('parseKey', () => {
  it('reads Camelot codes in any case', () => {
    expect(parseKey('8a')).toBe('8A');
    expect(parseKey(' 12B ')).toBe('12B');
    expect(parseKey('08A')).toBe('8A');
  });

  it('reads note names', () => {
    expect(parseKey('Am')).toBe('8A');
    expect(parseKey('C')).toBe('8B');
    expect(parseKey('F#m')).toBe('11A');
    expect(parseKey('Gb')).toBe('2B');
    expect(parseKey('Bbm')).toBe('3A');
    expect(parseKey('A# minor')).toBe('3A');
    expect(parseKey('Cmajor')).toBe('8B');
  });

  it('rejects junk', () => {
    expect(parseKey('13A')).toBeNull();
    expect(parseKey('0B')).toBeNull();
    expect(parseKey('8C')).toBeNull();
    expect(parseKey('')).toBeNull();
    expect(parseCamelot('hello')).toBeNull();
  });
});

describe('scrub models', () => {
  it('validates values', () => {
    expect(bpmScrub.valid(128)).toBe(true);
    expect(bpmScrub.valid(0)).toBe(false);
    expect(keyScrub.valid('8A')).toBe(true);
    expect(keyScrub.valid('—')).toBe(false);
  });

  it('formats for display and editing', () => {
    expect(bpmScrub.format(128)).toBe('128.00');
    expect(bpmScrub.editText(128.5)).toBe('128.50');
  });
});
