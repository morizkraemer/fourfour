import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ui, setOpenInspectorOnSelect, closeDetailPane } from './ui.svelte.ts';
import { select, toggle, selectRange, selectAll, setSelection, clear } from './selection.svelte.ts';

const list = [{ id: 1 }, { id: 2 }, { id: 3 }];

const memory = new Map<string, string>();

beforeEach(() => {
  memory.clear();
  vi.stubGlobal('localStorage', {
    getItem: (k: string) => memory.get(k) ?? null,
    setItem: (k: string, v: string) => void memory.set(k, v),
  });
  setOpenInspectorOnSelect(true);
  clear();
  closeDetailPane();
});

describe('inspector follows selection', () => {
  it('reopens a closed inspector on select', () => {
    select(1);
    expect(ui.detailPaneOpen).toBe(true);
  });

  it('reopens on range, select all and set selection', () => {
    selectRange(1, 2, list);
    expect(ui.detailPaneOpen).toBe(true);
    closeDetailPane();
    selectAll(list);
    expect(ui.detailPaneOpen).toBe(true);
    closeDetailPane();
    setSelection([2, 3]);
    expect(ui.detailPaneOpen).toBe(true);
  });

  it('reopens when a track is added by toggle but not when one is removed', () => {
    select(1);
    toggle(2);
    closeDetailPane();
    toggle(2);
    expect(ui.detailPaneOpen).toBe(false);
    toggle(3);
    expect(ui.detailPaneOpen).toBe(true);
  });

  it('stays closed for an empty selection and for clear', () => {
    setSelection([]);
    clear();
    expect(ui.detailPaneOpen).toBe(false);
  });

  it('stays closed when the setting is off, and persists the setting', () => {
    setOpenInspectorOnSelect(false);
    select(1);
    expect(ui.detailPaneOpen).toBe(false);
    expect(memory.get('fourfour.inspector.openOnSelect')).toBe('false');
  });
});
