import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  PLAYER_HEIGHT,
  clampPlayerHeight,
  defaultPlayerHeight,
  loadPlayerHeight,
  maxPlayerHeight,
  parseStoredPlayerHeight,
  savePlayerHeight,
} from './playerHeight.ts';

afterEach(() => vi.unstubAllGlobals());

describe('clampPlayerHeight', () => {
  it('keeps values between the minimum and 70 percent of the window', () => {
    expect(clampPlayerHeight(10, 900)).toBe(PLAYER_HEIGHT.min);
    expect(clampPlayerHeight(5000, 900)).toBe(630);
    expect(clampPlayerHeight(412.6, 900)).toBe(413);
  });
  it('never lets the maximum fall below the minimum in a tiny window', () => {
    expect(maxPlayerHeight(200)).toBe(PLAYER_HEIGHT.min);
    expect(clampPlayerHeight(500, 200)).toBe(PLAYER_HEIGHT.min);
  });
  it('falls back to the default for garbage', () => {
    expect(clampPlayerHeight(NaN, 900)).toBe(defaultPlayerHeight(900));
  });
});

describe('defaultPlayerHeight', () => {
  it('is a third of the window, at least 280', () => {
    expect(defaultPlayerHeight(1200)).toBe(396);
    expect(defaultPlayerHeight(600)).toBe(280);
  });
});

describe('parseStoredPlayerHeight', () => {
  it('means "use the default" when nothing valid is stored', () => {
    expect(parseStoredPlayerHeight(null)).toBeNull();
    expect(parseStoredPlayerHeight('abc')).toBeNull();
    expect(parseStoredPlayerHeight('12')).toBeNull();
  });
  it('reads a stored height', () => {
    expect(parseStoredPlayerHeight('420')).toBe(420);
  });
});

describe('persistence', () => {
  it('round-trips and clears through storage', () => {
    const store = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (k: string) => store.get(k) ?? null,
      setItem: (k: string, v: string) => void store.set(k, v),
      removeItem: (k: string) => void store.delete(k),
    });
    savePlayerHeight(444);
    expect(loadPlayerHeight()).toBe(444);
    savePlayerHeight(null);
    expect(loadPlayerHeight()).toBeNull();
  });
  it('survives unavailable storage', () => {
    const blocked = () => {
      throw new Error('blocked');
    };
    vi.stubGlobal('localStorage', { getItem: blocked, setItem: blocked, removeItem: blocked });
    expect(loadPlayerHeight()).toBeNull();
    expect(() => savePlayerHeight(300)).not.toThrow();
  });
});
