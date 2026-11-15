import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createAnalysisFocus, focusIds, type FocusTrack } from './analysis-focus.ts';

const rows = (count: number, analyzed: (id: number) => boolean = () => false): FocusTrack[] =>
  Array.from({ length: count }, (_, i) => ({ id: i + 1, analyzed: analyzed(i + 1), analysisPending: false }));

describe('focusIds', () => {
  it('lists the rows on screen top to bottom, then the band nearest first, below before above', () => {
    expect(focusIds(rows(20), 8, 11, 3)).toEqual([9, 10, 11, 12, 8, 13, 7, 14, 6]);
  });

  it('leaves out analysed tracks but keeps pending ones', () => {
    const tracks = rows(10, (id) => id % 2 === 0);
    tracks[3].analysisPending = true;
    expect(focusIds(tracks, 2, 6, 1)).toEqual([3, 4, 5, 7]);
  });

  it('stops at the ends of the list and at the limit', () => {
    expect(focusIds(rows(5), 0, 3, 10)).toEqual([1, 2, 3, 4, 5]);
    expect(focusIds(rows(5), 3, 99, 10)).toEqual([4, 5, 3, 2, 1]);
    expect(focusIds(rows(100), 0, 100, 0, 7)).toEqual([1, 2, 3, 4, 5, 6, 7]);
    expect(focusIds([], 0, 10)).toEqual([]);
  });
});

describe('createAnalysisFocus', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('sends the view as it is when the delay ends, once per change burst', () => {
    const send = vi.fn();
    const focus = createAnalysisFocus(send, 300);
    const tracks = rows(100);
    focus.request(tracks, 0, 3);
    vi.advanceTimersByTime(200);
    focus.request(tracks, 50, 53);
    expect(send).not.toHaveBeenCalled();
    vi.advanceTimersByTime(100);
    expect(send).toHaveBeenCalledTimes(1);
    expect(send.mock.calls[0][0].slice(0, 3)).toEqual([51, 52, 53]);
  });

  it('is not held back by changes that keep coming', () => {
    const send = vi.fn();
    const focus = createAnalysisFocus(send, 300);
    const tracks = rows(100);
    for (let i = 0; i < 10; i++) {
      focus.request(tracks, 10, 12);
      vi.advanceTimersByTime(250);
    }
    expect(send).toHaveBeenCalledTimes(1);
  });

  it('sends nothing when the ids did not change, and sends again when they did', () => {
    const send = vi.fn();
    const focus = createAnalysisFocus(send, 300);
    focus.request(rows(30), 10, 12);
    vi.advanceTimersByTime(300);
    focus.request(rows(30), 10, 12);
    vi.advanceTimersByTime(300);
    expect(send).toHaveBeenCalledTimes(1);
    focus.request(rows(30, (id) => id === 11), 10, 12);
    vi.advanceTimersByTime(300);
    expect(send).toHaveBeenCalledTimes(2);
    expect(send.mock.calls[1][0].slice(0, 2)).toEqual([12, 13]);
  });

  it('sends nothing after dispose', () => {
    const send = vi.fn();
    const focus = createAnalysisFocus(send, 300);
    focus.request(rows(10), 0, 3);
    focus.dispose();
    vi.advanceTimersByTime(1000);
    expect(send).not.toHaveBeenCalled();
  });
});
