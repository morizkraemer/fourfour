import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { loadProgressiveDetail, saveProgressiveDetail } from './progressiveDetail.ts';

describe('progressive detail setting', () => {
  beforeEach(() => {
    const store = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => store.get(key) ?? null,
      setItem: (key: string, value: string) => void store.set(key, value),
      removeItem: (key: string) => void store.delete(key),
    });
  });

  afterEach(() => vi.unstubAllGlobals());

  it('is off until it is switched on', () => {
    expect(loadProgressiveDetail()).toBe(false);
  });

  it('stays on across loads and goes off again when cleared', () => {
    saveProgressiveDetail(true);
    expect(loadProgressiveDetail()).toBe(true);
    saveProgressiveDetail(false);
    expect(loadProgressiveDetail()).toBe(false);
  });
});
