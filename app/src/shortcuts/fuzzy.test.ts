import { describe, expect, it } from 'vitest';
import { fuzzyMatch, fuzzyRank } from './fuzzy.ts';

describe('fuzzyMatch', () => {
  it('matches ordered subsequences case-insensitively and reports indices', () => {
    expect(fuzzyMatch('ppa', 'Play / pause')?.indices).toEqual([0, 7, 8]);
    expect(fuzzyMatch('zz', 'Play / pause')).toBeNull();
    expect(fuzzyMatch('pl', 'PLAY')?.indices).toEqual([0, 1]);
  });
  it('rejects out-of-order characters', () => {
    expect(fuzzyMatch('ba', 'ab')).toBeNull();
  });
  it('requires every token', () => {
    expect(fuzzyMatch('go set', 'Go to playlist Set A')).not.toBeNull();
    expect(fuzzyMatch('go xyz', 'Go to playlist Set A')).toBeNull();
  });
  it('matches everything on an empty query', () => {
    expect(fuzzyMatch('  ', 'anything')).toEqual({ score: 0, indices: [] });
  });
});

describe('fuzzyRank', () => {
  const labels = ['Open settings', 'Toggle player', 'New playlist', 'Sync stick', 'Analyze selection'];
  const rank = (q: string) => fuzzyRank(q, labels, (l) => l).map((r) => r.item);

  it('prefers prefix and word-start matches over scattered ones', () => {
    expect(rank('new')[0]).toBe('New playlist');
    expect(rank('sel')[0]).toBe('Analyze selection');
    expect(rank('set')[0]).toBe('Open settings');
  });
  it('prefers contiguous over scattered matches', () => {
    const contiguous = fuzzyMatch('stick', 'Sync stick')!.score;
    const scattered = fuzzyMatch('stick', 'Sync tracks quickly')!.score;
    expect(contiguous).toBeGreaterThan(scattered);
    expect(rank('play')).toEqual(expect.arrayContaining(['Toggle player', 'New playlist']));
  });
  it('drops non-matches', () => {
    expect(rank('qqq')).toEqual([]);
  });
});
