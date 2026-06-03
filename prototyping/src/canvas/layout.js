/*
 * Deterministic auto-grid. Artboards are grouped into one column per layer
 * (primitive | module | composite), stacked top-to-bottom in each column.
 * Saved positions (manual drags) override the computed slot.
 */
export const LAYER_ORDER = ['primitive', 'module', 'composite'];

const COL_X = 48; // first column x
const COL_GAP = 520; // horizontal gap between layer columns
const ROW_Y = 48; // first row y
const ROW_GAP = 360; // vertical gap between cards in a column

export function autoGrid(registry, savedPositions = {}) {
  const counts = {}; // per-layer row counter
  return registry.map((entry) => {
    const colIndex = Math.max(0, LAYER_ORDER.indexOf(entry.layer));
    const rowIndex = counts[entry.layer] ?? 0;
    counts[entry.layer] = rowIndex + 1;
    const slot = {
      x: COL_X + colIndex * COL_GAP,
      y: ROW_Y + rowIndex * ROW_GAP,
    };
    return { ...entry, position: savedPositions[entry.id] || slot };
  });
}
