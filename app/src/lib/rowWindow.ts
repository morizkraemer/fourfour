export const ROW_HEIGHT = 28;
export const OVERSCAN_ROWS = 12;
export const MIN_TRAILING_ROWS = 4;
export const LEAD_STEP_ROWS = 4;
export const LEAD_LOOKAHEAD_MS = 60;

export interface RowWindowInput {
  scrollTop: number;
  viewportHeight: number;
  headerHeight: number;
  rowCount: number;
  rowHeight?: number;
  overscan?: number;
  /** Signed scroll speed in px per ms; the window leans its overscan into the direction of travel. */
  velocity?: number;
}

export interface RowWindow {
  firstRow: number;
  endRow: number;
  poolSize: number;
}

export function rowWindow({
  scrollTop,
  viewportHeight,
  headerHeight,
  rowCount,
  rowHeight = ROW_HEIGHT,
  overscan = OVERSCAN_ROWS,
  velocity = 0,
}: RowWindowInput): RowWindow {
  const lead = leadRows(velocity, overscan, rowHeight);
  const trail = 2 * overscan - lead;
  const above = velocity < 0 ? lead : trail;
  const below = velocity < 0 ? trail : lead;
  return {
    firstRow: Math.max(0, Math.floor((scrollTop - headerHeight) / rowHeight) - above),
    endRow: Math.min(rowCount, Math.ceil((scrollTop + viewportHeight - headerHeight) / rowHeight) + below),
    poolSize: Math.ceil(viewportHeight / rowHeight) + 2 * overscan + 2,
  };
}

/**
 * Overscan rows ahead of the scroll direction: enough for the distance the scroller travels in
 * `LEAD_LOOKAHEAD_MS` (a threaded scroller keeps moving while the page builds the next rows), in
 * steps of `LEAD_STEP_ROWS` so the window does not shift on every small speed change. The pool size
 * stays the same; the trailing side gives up the rows.
 */
export function leadRows(velocity: number, overscan = OVERSCAN_ROWS, rowHeight = ROW_HEIGHT): number {
  const most = Math.max(overscan, 2 * overscan - MIN_TRAILING_ROWS);
  const travel = Math.ceil((Math.abs(velocity) * LEAD_LOOKAHEAD_MS) / rowHeight);
  const extra = Math.ceil(travel / LEAD_STEP_ROWS) * LEAD_STEP_ROWS;
  return Math.min(most, overscan + extra);
}

/** Row index shown by pool slot `slot`: slots keep their row while the window slides. */
export function indexForSlot(slot: number, firstRow: number, poolSize: number): number {
  return firstRow + ((((slot - firstRow) % poolSize) + poolSize) % poolSize);
}

export function rowTop(index: number, rowHeight = ROW_HEIGHT): number {
  return index * rowHeight;
}

export function spacerHeight(rowCount: number, rowHeight = ROW_HEIGHT): number | undefined {
  return rowCount > 0 ? rowCount * rowHeight : undefined;
}

export function dropLineTop(index: number, rowCount: number, rowHeight = ROW_HEIGHT): number {
  return Math.min(index, rowCount) * rowHeight;
}

/** scrollTop that brings row `index` fully into view, or the current one if it already is. */
export function scrollTopToReveal(
  index: number,
  { scrollTop, viewportHeight, headerHeight, rowCount, rowHeight = ROW_HEIGHT }: Omit<RowWindowInput, 'overscan'>,
): number {
  if (rowCount === 0) return 0;
  const row = Math.min(Math.max(index, 0), rowCount - 1);
  const rowStart = headerHeight + row * rowHeight;
  const rowEnd = rowStart + rowHeight;
  const maxScroll = Math.max(0, headerHeight + rowCount * rowHeight - viewportHeight);
  if (rowStart < scrollTop + headerHeight) return Math.min(maxScroll, Math.max(0, rowStart - headerHeight));
  if (rowEnd > scrollTop + viewportHeight) return Math.min(maxScroll, rowEnd - viewportHeight);
  return scrollTop;
}
