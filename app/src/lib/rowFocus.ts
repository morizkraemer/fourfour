export const LEAD_FRAMES = 14;
export const MAX_LEAD_ROWS = 160;
export const TRAIL_ROWS = 12;

export interface RowFocusInput {
  firstRow: number;
  endRow: number;
  visibleFirst: number;
  visibleLast: number;
  rowCount: number;
  rowsPerFrame: number;
}

export function leadRows(rowsPerFrame: number): number {
  return Math.min(MAX_LEAD_ROWS, Math.round(Math.abs(rowsPerFrame) * LEAD_FRAMES));
}

const MOVING_ROWS_PER_FRAME = 0.5;

/**
 * Row indexes whose data (previews, covers) should be loaded, in the order to load them: closest first to
 * where the user will be looking by the time the data arrives. Standing still that is the middle of the screen;
 * scrolling it is half the lead ahead of the edge the scroll is heading for.
 */
export function rowFocusOrder({ firstRow, endRow, visibleFirst, visibleLast, rowCount, rowsPerFrame }: RowFocusInput): number[] {
  const down = rowsPerFrame >= 0;
  const lead = leadRows(rowsPerFrame);
  const from = Math.max(0, Math.min(firstRow, down ? visibleFirst - TRAIL_ROWS : visibleFirst - lead));
  const to = Math.min(rowCount, Math.max(endRow, down ? visibleLast + 1 + lead : visibleLast + 1 + TRAIL_ROWS));
  const moving = Math.abs(rowsPerFrame) >= MOVING_ROWS_PER_FRAME;
  const aim = !moving ? (visibleFirst + visibleLast) / 2 : down ? visibleLast + lead / 2 : visibleFirst - lead / 2;
  const order: number[] = [];
  for (let i = from; i < to; i++) order.push(i);
  return order.sort((a, b) => Math.abs(a - aim) - Math.abs(b - aim) || a - b);
}
