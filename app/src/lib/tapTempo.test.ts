import { describe, expect, it } from 'vitest';
import { createTapTempo } from './tapTempo.ts';

function tapper(times: number[]) {
  let clock = 0;
  const tempo = createTapTempo(() => clock);
  return times.map((t) => {
    clock = t;
    return tempo.tap();
  });
}

describe('createTapTempo', () => {
  it('gives nothing before the fourth tap, then the BPM of the average interval', () => {
    expect(tapper([0, 500, 1000, 1500, 2000])).toEqual([null, null, null, 120, 120]);
  });

  it('averages uneven taps', () => {
    expect(tapper([0, 480, 1000, 1520])[3]).toBe(118.42);
  });

  it('starts over after 2 s without a tap', () => {
    expect(tapper([0, 500, 1000, 1500, 4000, 4400, 4800, 5200])).toEqual([null, null, null, 120, null, null, null, 150]);
  });
});
