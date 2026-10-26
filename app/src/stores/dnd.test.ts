import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { dnd, endDrag, startTrackDrag, updateTrackDropTarget } from './dnd.svelte.ts';

interface FakeRow {
  dataset: { rowIndex: string };
  getBoundingClientRect: () => { top: number; height: number };
}

const row = (index: number, top: number, height = 28): FakeRow => ({
  dataset: { rowIndex: String(index) },
  getBoundingClientRect: () => ({ top, height }),
});

function stubDocument(rows: FakeRow[] | null, sourceId: string | null = 'All Tracks') {
  const body = {
    getAttribute: (name: string) => (name === 'data-source-id' ? sourceId : null),
    querySelectorAll: () => rows ?? [],
  };
  vi.stubGlobal('document', {
    elementFromPoint: () => (rows === null ? null : { closest: () => body }),
  });
}

beforeEach(() => {
  startTrackDrag([1, 2], '2 tracks', null, 'All Tracks', 0, 0);
});

afterEach(() => {
  endDrag();
  vi.unstubAllGlobals();
});

describe('updateTrackDropTarget', () => {
  it('picks the lowest row whose midpoint is below the pointer, regardless of DOM order', () => {
    stubDocument([row(7, 196), row(5, 140), row(6, 168), row(4, 112)]);
    updateTrackDropTarget(10, 150);
    expect(dnd.dropRowIndex).toBe(5);
  });
  it('lands before the first row when the pointer is above everything', () => {
    stubDocument([row(10, 280), row(11, 308)]);
    updateTrackDropTarget(10, 0);
    expect(dnd.dropRowIndex).toBe(10);
  });
  it('appends after the highest rendered row when the pointer is below all rows', () => {
    stubDocument([row(3, 84), row(5, 140), row(4, 112)]);
    updateTrackDropTarget(10, 1000);
    expect(dnd.dropRowIndex).toBe(6);
  });
  it('uses the row midpoint as the boundary', () => {
    stubDocument([row(0, 0, 28), row(1, 28, 28)]);
    updateTrackDropTarget(10, 13);
    expect(dnd.dropRowIndex).toBe(0);
    updateTrackDropTarget(10, 14);
    expect(dnd.dropRowIndex).toBe(1);
  });
  it('drops at index 0 on an empty body and records the panel', () => {
    stubDocument([], 'Warmup');
    updateTrackDropTarget(10, 10);
    expect(dnd.dropRowIndex).toBe(0);
    expect(dnd.dropPanelSourceId).toBe('Warmup');
  });
  it('clears targets when nothing is under the pointer', () => {
    stubDocument([row(0, 0)]);
    updateTrackDropTarget(10, 5);
    stubDocument(null);
    updateTrackDropTarget(10, 5);
    expect(dnd.dropRowIndex).toBeNull();
    expect(dnd.dropPanelSourceId).toBeNull();
  });
  it('clears targets when the body has no source id', () => {
    stubDocument([row(0, 0)], null);
    updateTrackDropTarget(10, 5);
    expect(dnd.dropPanelSourceId).toBeNull();
    expect(dnd.dropRowIndex).toBeNull();
  });
  it('ignores pointer moves when no track drag is active', () => {
    endDrag();
    stubDocument([row(0, 0)]);
    updateTrackDropTarget(10, 5);
    expect(dnd.dropRowIndex).toBeNull();
    expect(dnd.dropPanelSourceId).toBeNull();
  });
});

describe('drag session', () => {
  it('startTrackDrag records the payload and endDrag resets it', () => {
    expect(dnd.kind).toBe('tracks');
    expect(dnd.trackIds).toEqual([1, 2]);
    expect(dnd.ghostCount).toBe(2);
    endDrag();
    expect(dnd.kind).toBeNull();
    expect(dnd.trackIds).toEqual([]);
    expect(dnd.dropRowIndex).toBeNull();
  });
});
