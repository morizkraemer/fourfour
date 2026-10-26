import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { dnd, endDrag, startTrackDrag, updateTrackDropTarget } from './dnd.svelte.ts';

function stubDocument(body: { top: number; rowCount: number } | null, sourceId: string | null = 'All Tracks') {
  const el = {
    getAttribute: (name: string) =>
      name === 'data-source-id' ? sourceId : name === 'data-row-count' ? String(body?.rowCount ?? 0) : null,
    getBoundingClientRect: () => ({ top: body?.top ?? 0 }),
  };
  vi.stubGlobal('document', {
    elementFromPoint: () => (body === null ? null : { closest: () => el }),
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
  it('picks the lowest row whose midpoint is below the pointer', () => {
    stubDocument({ top: 0, rowCount: 100 });
    updateTrackDropTarget(10, 150);
    expect(dnd.dropRowIndex).toBe(5);
  });
  it('follows the body position when the list is scrolled', () => {
    stubDocument({ top: -280, rowCount: 100 });
    updateTrackDropTarget(10, 0);
    expect(dnd.dropRowIndex).toBe(10);
  });
  it('lands before the first row when the pointer is above everything', () => {
    stubDocument({ top: 50, rowCount: 100 });
    updateTrackDropTarget(10, 0);
    expect(dnd.dropRowIndex).toBe(0);
  });
  it('appends after the last row when the pointer is below all rows', () => {
    stubDocument({ top: 0, rowCount: 6 });
    updateTrackDropTarget(10, 1000);
    expect(dnd.dropRowIndex).toBe(6);
  });
  it('uses the row midpoint as the boundary', () => {
    stubDocument({ top: 0, rowCount: 10 });
    updateTrackDropTarget(10, 13);
    expect(dnd.dropRowIndex).toBe(0);
    updateTrackDropTarget(10, 14);
    expect(dnd.dropRowIndex).toBe(1);
  });
  it('drops at index 0 on an empty body and records the panel', () => {
    stubDocument({ top: 0, rowCount: 0 }, 'Warmup');
    updateTrackDropTarget(10, 10);
    expect(dnd.dropRowIndex).toBe(0);
    expect(dnd.dropPanelSourceId).toBe('Warmup');
  });
  it('clears targets when nothing is under the pointer', () => {
    stubDocument({ top: 0, rowCount: 1 });
    updateTrackDropTarget(10, 5);
    stubDocument(null);
    updateTrackDropTarget(10, 5);
    expect(dnd.dropRowIndex).toBeNull();
    expect(dnd.dropPanelSourceId).toBeNull();
  });
  it('clears targets when the body has no source id', () => {
    stubDocument({ top: 0, rowCount: 1 }, null);
    updateTrackDropTarget(10, 5);
    expect(dnd.dropPanelSourceId).toBeNull();
    expect(dnd.dropRowIndex).toBeNull();
  });
  it('ignores pointer moves when no track drag is active', () => {
    endDrag();
    stubDocument({ top: 0, rowCount: 1 });
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
