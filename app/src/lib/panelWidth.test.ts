import { afterEach, describe, expect, it, vi } from 'vitest';
import { clampPanelWidth, loadPanelWidth, parseStoredPanelWidth, PANEL_WIDTH, savePanelWidth } from './panelWidth.ts';

afterEach(() => vi.unstubAllGlobals());

describe('clampPanelWidth', () => {
  it('keeps values inside the limits', () => {
    expect(clampPanelWidth('sidebar', 10)).toBe(PANEL_WIDTH.sidebar.min);
    expect(clampPanelWidth('sidebar', 9999)).toBe(PANEL_WIDTH.sidebar.max);
    expect(clampPanelWidth('inspector', 300.4)).toBe(300);
  });
  it('falls back to the initial width for garbage', () => {
    expect(clampPanelWidth('inspector', NaN)).toBe(PANEL_WIDTH.inspector.initial);
  });
});

describe('parseStoredPanelWidth', () => {
  it('uses the initial width when nothing is stored', () => {
    expect(parseStoredPanelWidth('sidebar', null)).toBe(240);
    expect(parseStoredPanelWidth('sidebar', 'abc')).toBe(240);
  });
  it('clamps stored values', () => {
    expect(parseStoredPanelWidth('inspector', '5000')).toBe(PANEL_WIDTH.inspector.max);
  });
});

describe('persistence', () => {
  it('round-trips through storage', () => {
    const store = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (k: string) => store.get(k) ?? null,
      setItem: (k: string, v: string) => void store.set(k, v),
    });
    savePanelWidth('sidebar', 333);
    expect(loadPanelWidth('sidebar')).toBe(333);
    expect(loadPanelWidth('inspector')).toBe(278);
  });
  it('survives unavailable storage', () => {
    const blocked = () => {
      throw new Error('blocked');
    };
    vi.stubGlobal('localStorage', { getItem: blocked, setItem: blocked });
    expect(loadPanelWidth('sidebar')).toBe(240);
    expect(() => savePanelWidth('sidebar', 300)).not.toThrow();
  });
});
