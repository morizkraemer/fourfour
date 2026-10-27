import { describe, expect, it } from 'vitest';
import { formatBoardDuration, summarizeBoard } from './boardSummary.ts';

describe('summarizeBoard', () => {
  it('averages analysed BPMs and spans the keys', () => {
    const summary = summarizeBoard([
      { bpmExact: 124, key: '9A', raw: { duration_secs: 300 } },
      { bpmExact: 128, key: '8A', raw: { duration_secs: 360 } },
      { bpmExact: 0, key: '—' },
    ]);
    expect(summary).toEqual({ count: 3, durationSecs: 660, avgBpm: 126, keyRange: '8A–9A' });
  });

  it('reports nothing for an empty board', () => {
    expect(summarizeBoard([])).toEqual({ count: 0, durationSecs: 0, avgBpm: null, keyRange: null });
  });
});

describe('formatBoardDuration', () => {
  it('uses hours only from an hour up', () => {
    expect(formatBoardDuration(2400)).toBe('40m');
    expect(formatBoardDuration(5100)).toBe('1h 25m');
  });
});
