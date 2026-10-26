export const ROW_HEIGHT = 28;
export const OVERSCAN_ROWS = 12;

export interface RowWindowInput {
  scrollTop: number;
  viewportHeight: number;
  headerHeight: number;
  rowCount: number;
  rowHeight?: number;
  overscan?: number;
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
}: RowWindowInput): RowWindow {
  return {
    firstRow: Math.max(0, Math.floor((scrollTop - headerHeight) / rowHeight) - overscan),
    endRow: Math.min(rowCount, Math.ceil((scrollTop + viewportHeight - headerHeight) / rowHeight) + overscan),
    poolSize: Math.ceil(viewportHeight / rowHeight) + 2 * overscan + 2,
  };
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
