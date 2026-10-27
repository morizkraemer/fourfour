import { describe, expect, it } from 'vitest';
import {
  CUE_PALETTE,
  cueChip,
  cueColor,
  cueKey,
  cueMarkers,
  cuesFromPayload,
  cuesToPayload,
  firstFreeSlot,
  formatCueTime,
  initialCuePointMs,
  sameCues,
  slotLetter,
  withCueColor,
  withCuePad,
  withCueName,
  withHotCue,
  withMemoryCue,
  withoutCue,
  type Cue,
} from './cues.ts';

function cue(slot: number, timeMs: number, extra: Partial<Cue> = {}): Cue {
  return { slot, timeMs, loopMs: null, color: null, name: '', ...extra };
}

describe('slots and keys', () => {
  it('names hot cue slots A to H and memory cues M', () => {
    expect([1, 2, 8].map(slotLetter)).toEqual(['A', 'B', 'H']);
    expect(slotLetter(0)).toBe('M');
  });

  it('keys a hot cue by slot and a memory cue by time', () => {
    expect(cueKey(cue(3, 9000))).toBe('h3');
    expect(cueKey(cue(0, 9000))).toBe('m9000');
  });
});

describe('payload mapping', () => {
  it('sorts hot cues by slot before memory cues by time', () => {
    const cues = cuesFromPayload([
      { hot_cue_number: 0, time_ms: 5000 },
      { hot_cue_number: 2, time_ms: 1000, name: 'Drop', color: '#ffffff', loop_time_ms: 2000 },
      { hot_cue_number: 0, time_ms: 1000 },
      { hot_cue_number: 1, time_ms: 9000 },
    ]);
    expect(cues.map(cueKey)).toEqual(['h1', 'h2', 'm1000', 'm5000']);
    expect(cues[1]).toMatchObject({ name: 'Drop', color: '#ffffff', loopMs: 2000 });
  });

  it('round-trips through the payload', () => {
    const cues = [cue(1, 100, { name: 'In', color: '#112233', loopMs: 400 }), cue(0, 200)];
    expect(cuesFromPayload(cuesToPayload(cues))).toEqual(cues);
  });

  it('takes a missing list as no cues', () => {
    expect(cuesFromPayload(undefined)).toEqual([]);
  });

  it('compares lists by content', () => {
    expect(sameCues([cue(1, 5)], [cue(1, 5)])).toBe(true);
    expect(sameCues([cue(1, 5)], [cue(1, 5, { name: 'x' })])).toBe(false);
    expect(sameCues([cue(1, 5)], [])).toBe(false);
  });
});

describe('setting hot cues', () => {
  it('uses the first free slot', () => {
    expect(firstFreeSlot([])).toBe(1);
    expect(firstFreeSlot([cue(1, 0), cue(2, 0), cue(4, 0)])).toBe(3);
  });

  it('adds a hot cue in the first free slot', () => {
    const first = withHotCue([], 75_400)!;
    expect(first.cue).toMatchObject({ slot: 1, timeMs: 75_400 });
    const second = withHotCue(first.cues, 90_000)!;
    expect(second.cues.map(cueKey)).toEqual(['h1', 'h2']);
  });

  it('fills a gap left by a deleted cue', () => {
    const cues = [cue(1, 0), cue(3, 0)];
    expect(withHotCue(cues, 10)!.cue.slot).toBe(2);
  });

  it('refuses a taken slot and a full set', () => {
    expect(withHotCue([cue(2, 0)], 10, 2)).toBeNull();
    const full = Array.from({ length: 8 }, (_, i) => cue(i + 1, i));
    expect(firstFreeSlot(full)).toBeNull();
    expect(withHotCue(full, 10)).toBeNull();
  });

  it('refuses slots outside A to H', () => {
    expect(withHotCue([], 10, 0)).toBeNull();
    expect(withHotCue([], 10, 9)).toBeNull();
  });

  it('leaves memory cues out of the slot search', () => {
    expect(firstFreeSlot([cue(0, 5), cue(0, 9)])).toBe(1);
  });
});

describe('memory cues', () => {
  it('adds in time order', () => {
    const added = withMemoryCue([cue(0, 9000)], 2000);
    expect(added.added).toBe(true);
    expect(added.cues.map((c) => c.timeMs)).toEqual([2000, 9000]);
  });

  it('does not add a second cue at the same spot', () => {
    const start = [cue(0, 2000)];
    const again = withMemoryCue(start, 2030);
    expect(again.added).toBe(false);
    expect(again.cues).toBe(start);
  });

  it('allows a memory cue where a hot cue is', () => {
    expect(withMemoryCue([cue(1, 2000)], 2000).added).toBe(true);
  });
});

describe('editing', () => {
  const cues = [cue(1, 1000), cue(0, 3000), cue(0, 4000)];

  it('deletes one cue by key', () => {
    expect(withoutCue(cues, 'h1').map(cueKey)).toEqual(['m3000', 'm4000']);
    expect(withoutCue(cues, 'm3000').map(cueKey)).toEqual(['h1', 'm4000']);
  });

  it('renames and trims the name', () => {
    const renamed = withCueName(cues, 'h1', '  Intro  ');
    expect(renamed[0].name).toBe('Intro');
    expect(renamed[1].name).toBe('');
    expect(withCueName(cues, 'h1', 'x'.repeat(80))[0].name).toHaveLength(32);
  });

  it('recolours one cue', () => {
    const recoloured = withCueColor(cues, 'm4000', '#ff0000');
    expect(recoloured[2].color).toBe('#ff0000');
    expect(recoloured[0].color).toBeNull();
  });

  it('does not touch the list it was given', () => {
    withCueName(cues, 'h1', 'Intro');
    expect(cues[0].name).toBe('');
  });
});

describe('colour', () => {
  it('uses a stored hex colour as it is', () => {
    expect(cueColor(cue(1, 0, { color: '#AbCdEf' }))).toBe('#AbCdEf');
  });

  it('maps an imported palette index to the palette', () => {
    expect(cueColor(cue(1, 0, { color: 'table:2' }))).toBe(CUE_PALETTE[1].color);
    expect(cueColor(cue(1, 0, { color: 'table:17' }))).toBe(CUE_PALETTE[0].color);
  });

  it('gives each hot slot its own default and memory cues red', () => {
    const colors = Array.from({ length: 8 }, (_, i) => cueColor(cue(i + 1, 0)));
    expect(new Set(colors).size).toBe(8);
    expect(cueColor(cue(0, 0))).toBe('#ff3b30');
  });

  it('has sixteen distinct palette colours', () => {
    expect(new Set(CUE_PALETTE.map((p) => p.color)).size).toBe(16);
  });
});

describe('chip label', () => {
  it('shows slot letter, colour and time', () => {
    const chip = cueChip(cue(8, 75_000));
    expect(chip).toMatchObject({ letter: 'H', time: '1:15', name: '', label: 'H 1:15' });
    expect(chip.color).toBe(cueColor(cue(8, 0)));
  });

  it('adds the name when there is one', () => {
    expect(cueChip(cue(1, 90_000, { name: 'Vocal in' })).label).toBe('A Vocal in 1:30');
  });

  it('labels a memory cue M with its time', () => {
    expect(cueChip(cue(0, 60_000)).label).toBe('1:00');
    expect(cueChip(cue(0, 60_000)).padded).toBe(false);
    expect(cueChip(cue(2, 60_000)).padded).toBe(true);
  });

  it('never cuts the label short', () => {
    for (let slot = 0; slot <= 8; slot++) {
      expect(cueChip(cue(slot, 3_599_000, { name: 'A fairly long cue name' })).label).not.toContain('..');
    }
  });

  it('formats times as m:ss', () => {
    expect(formatCueTime(0)).toBe('0:00');
    expect(formatCueTime(59_999)).toBe('0:59');
    expect(formatCueTime(61_000)).toBe('1:01');
    expect(formatCueTime(3_725_000)).toBe('62:05');
    expect(formatCueTime(-5)).toBe('0:00');
  });
});

describe('markers', () => {
  it('places markers as a fraction of the track with the slot letter', () => {
    const markers = cueMarkers([cue(1, 25_000), cue(0, 50_000)], 100_000);
    expect(markers.map((m) => m.position)).toEqual([0.25, 0.5]);
    expect(markers.map((m) => m.label)).toEqual(['A', '']);
    expect(markers.map((m) => m.padded)).toEqual([true, false]);
    expect(markers.map((m) => m.key)).toEqual(['h1', 'm50000']);
  });

  it('draws none without a duration and keeps late cues on the strip', () => {
    expect(cueMarkers([cue(1, 5)], 0)).toEqual([]);
    expect(cueMarkers([cue(1, 200_000)], 100_000)[0].position).toBe(1);
  });
});

describe('cue point', () => {
  it('starts at the first memory cue, else the start', () => {
    expect(initialCuePointMs([cue(1, 5000)])).toBe(0);
    expect(initialCuePointMs([cue(1, 5000), cue(0, 2000), cue(0, 9000)])).toBe(2000);
  });
});

describe('assigning a pad', () => {
  it('gives a cue without a pad the pad and keeps name and colour', () => {
    const cues = [cue(0, 9000, { name: 'Out', color: '#ff3b30' })];
    expect(withCuePad(cues, 'm9000', 3)).toEqual([cue(3, 9000, { name: 'Out', color: '#ff3b30' })]);
  });

  it('moves a padded cue to a free pad', () => {
    const cues = [cue(1, 1000), cue(2, 2000)];
    expect(withCuePad(cues, 'h1', 5)!.map((c) => [c.slot, c.timeMs])).toEqual([[2, 2000], [5, 1000]]);
  });

  it('refuses a taken pad, an out of range pad and an unknown cue', () => {
    const cues = [cue(1, 1000), cue(0, 2000)];
    expect(withCuePad(cues, 'm2000', 1)).toBeNull();
    expect(withCuePad(cues, 'm2000', 9)).toBeNull();
    expect(withCuePad(cues, 'h7', 2)).toBeNull();
  });

  it('removing the pad keeps the cue at its position without one, keeping name and colour', () => {
    const cues = [cue(2, 5000, { name: 'Drop', color: '#ff3b30' })];
    expect(withCuePad(cues, 'h2', null)).toEqual([cue(0, 5000, { name: 'Drop', color: '#ff3b30' })]);
  });

  it('removing the pad folds into a cue without a pad already there', () => {
    expect(withCuePad([cue(1, 5000), cue(0, 5020)], 'h1', null)).toEqual([cue(0, 5020)]);
  });

  it('removing the pad of a cue that has none changes nothing', () => {
    const cues = [cue(0, 5000)];
    expect(withCuePad(cues, 'm5000', null)).toBe(cues);
  });
});
