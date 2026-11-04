export type SortValue = string | number | null;

/** Positions in sorted order, and the inverse: `rank[order[i]] === i`. */
export interface SortedOrder {
  order: Int32Array;
  rank: Int32Array;
}

export function compareSortValues(a: SortValue, b: SortValue, direction: 1 | -1): number {
  if (a === null) return b === null ? 0 : 1;
  if (b === null) return -1;
  if (a < b) return -direction;
  if (a > b) return direction;
  return 0;
}

function ranksFromOrder(order: Int32Array): Int32Array {
  const rank = new Int32Array(order.length);
  for (let i = 0; i < order.length; i++) rank[order[i]] = i;
  return rank;
}

function uniformKind(values: readonly SortValue[]): 'string' | 'number' | 'mixed' | 'none' {
  let kind: 'string' | 'number' | 'mixed' | 'none' = 'none';
  for (let i = 0; i < values.length; i++) {
    const v = values[i];
    if (v === null) continue;
    const next = typeof v === 'string' ? 'string' : 'number';
    if (kind === 'none') kind = next;
    else if (kind !== next) return 'mixed';
  }
  return kind;
}

/**
 * The order `Array.prototype.sort` gives positions compared by value: `direction` 1 ascending, -1 descending, no value
 * last in both, equal values in position order. One native numeric sort over `rank * n + position` instead of a
 * comparator call per comparison.
 */
export function sortedOrder(values: readonly SortValue[], direction: 1 | -1): SortedOrder {
  const n = values.length;
  const kind = uniformKind(values);
  if (kind === 'mixed') {
    const order = Int32Array.from({ length: n }, (_, i) => i).sort((a, b) => compareSortValues(values[a], values[b], direction) || a - b);
    return { order, rank: ranksFromOrder(order) };
  }
  const distinct = new Set<string | number>();
  for (let i = 0; i < n; i++) {
    const v = values[i];
    if (v !== null) distinct.add(v);
  }
  const list = [...distinct];
  if (kind === 'number') (list as number[]).sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
  else list.sort();
  const rankOfValue = new Map<string | number, number>();
  for (let i = 0; i < list.length; i++) rankOfValue.set(list[i], direction === 1 ? i : list.length - 1 - i);
  const keys = new Float64Array(n);
  for (let i = 0; i < n; i++) {
    const v = values[i];
    keys[i] = (v === null ? list.length : rankOfValue.get(v)!) * n + i;
  }
  keys.sort();
  const order = new Int32Array(n);
  for (let i = 0; i < n; i++) order[i] = keys[i] % n;
  return { order, rank: ranksFromOrder(order) };
}

/** `positions` (any order) sorted by their place in `sorted`; the same as sorting just those positions. */
export function sortedSubset(positions: ArrayLike<number>, sorted: SortedOrder): Int32Array {
  const keys = new Int32Array(positions.length);
  for (let i = 0; i < positions.length; i++) keys[i] = sorted.rank[positions[i]];
  keys.sort();
  for (let i = 0; i < keys.length; i++) keys[i] = sorted.order[keys[i]];
  return keys;
}

/** Most positions `withChangedValues` repositions one by one before a new sort is cheaper. */
export const MAX_REPOSITIONED = 64;

/**
 * `sorted` after the values at `changed` positions were replaced in `values` (already written): the same order a new
 * `sortedOrder(values, direction)` gives, by moving only those positions. `null` when more than `MAX_REPOSITIONED` moved.
 */
export function withChangedValues(sorted: SortedOrder, values: readonly SortValue[], changed: readonly number[], direction: 1 | -1): SortedOrder | null {
  if (changed.length === 0) return sorted;
  if (changed.length > MAX_REPOSITIONED) return null;
  const n = sorted.order.length;
  const moving = [...new Set(changed)];
  const isMoving = new Uint8Array(n);
  for (const p of moving) isMoving[p] = 1;
  const order = new Int32Array(n);
  let kept = 0;
  for (let i = 0; i < n; i++) {
    const p = sorted.order[i];
    if (!isMoving[p]) order[kept++] = p;
  }
  const before = (position: number, value: SortValue, other: number) => {
    const c = compareSortValues(values[other], value, direction);
    return c < 0 || (c === 0 && other < position);
  };
  for (const p of moving.sort((a, b) => a - b)) {
    let low = 0;
    let high = kept;
    while (low < high) {
      const mid = (low + high) >>> 1;
      if (before(p, values[p], order[mid])) low = mid + 1;
      else high = mid;
    }
    order.copyWithin(low + 1, low, kept);
    order[low] = p;
    kept++;
  }
  return { order, rank: ranksFromOrder(order) };
}
