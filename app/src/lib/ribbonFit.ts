export const RIBBON_PADDING_PX = 6;
export const RIBBON_TOP_PX = 28;
export const RIBBON_TILE_PX = 40;
export const RIBBON_GAP_PX = 6;

export interface RibbonFit {
  covers: string[];
  more: number;
}

export function ribbonSlots(heightPx: number): number {
  const room = heightPx - RIBBON_PADDING_PX * 2 - RIBBON_TOP_PX;
  return Math.max(0, Math.floor(room / (RIBBON_TILE_PX + RIBBON_GAP_PX)));
}

export function fitRibbonCovers(covers: string[], heightPx: number, reservedSlots = 0): RibbonFit {
  const slots = Math.max(0, ribbonSlots(heightPx) - reservedSlots);
  if (covers.length <= slots) return { covers, more: 0 };
  const shown = Math.max(0, slots - 1);
  return { covers: covers.slice(0, shown), more: covers.length - shown };
}
