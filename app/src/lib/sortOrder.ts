export type SortValue = string | number | null;

/** Positions in sorted order, and the inverse (`rank[order[i]] === i`, built when first read and again after `order` moved in place). */
export class SortedOrder {
  readonly order: Int32Array;
  private ranks: Int32Array | null = null;

  constructor(order: Int32Array) {
    this.order = order;
  }

  get rank(): Int32Array {
    if (this.ranks === null) this.ranks = ranksFromOrder(this.order);
    return this.ranks;
  }

  rankIsBuilt(): boolean {
    return this.ranks !== null;
  }

  orderMoved() {
    this.ranks = null;
  }
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

/** Where each value sits among the distinct values: `classes[p]` 0 for the lowest, `count` for no value. */
export interface ValueClasses {
  classes: Int32Array;
  count: number;
}

const distinctSorted = new WeakMap<ValueClasses, (string | number)[]>();

function* mergeRuns(a: string[], b: string[], step: number): Generator<void, string[]> {
  const out: string[] = new Array(a.length + b.length);
  let i = 0;
  let j = 0;
  let k = 0;
  while (i < a.length && j < b.length) {
    const limit = k + step;
    while (k < limit && i < a.length && j < b.length) out[k++] = a[i] < b[j] ? a[i++] : b[j++];
    yield;
  }
  while (i < a.length) out[k++] = a[i++];
  while (j < b.length) out[k++] = b[j++];
  return out;
}

function* sortDistinctStrings(list: string[], chunk: number): Generator<void, string[]> {
  if (list.length <= chunk) return list.sort();
  let runs: string[][] = [];
  for (let start = 0; start < list.length; start += chunk) {
    runs.push(list.slice(start, start + chunk).sort());
    yield;
  }
  while (runs.length > 1) {
    const merged: string[][] = [];
    for (let r = 0; r + 1 < runs.length; r += 2) merged.push(yield* mergeRuns(runs[r], runs[r + 1], chunk));
    if (runs.length % 2 === 1) merged.push(runs[runs.length - 1]);
    runs = merged;
  }
  return runs[0];
}

/**
 * The classes of `values` (equal values share one), in steps of about `chunk` values or distinct strings so a caller can
 * stop between them; `chunk` `Infinity` runs it as one piece. Ends with `null` for values that are not all strings or
 * all numbers (or hold a NaN), which have no stable order of classes.
 */
export function* valueClassesSteps(values: readonly SortValue[], chunk: number): Generator<void, ValueClasses | null> {
  const n = values.length;
  const ids = new Map<string | number, number>();
  const distinct: (string | number)[] = [];
  const classes = new Int32Array(n);
  let kind: 'string' | 'number' | null = null;
  for (let start = 0; start < n; start += chunk) {
    const end = Math.min(n, start + chunk);
    for (let i = start; i < end; i++) {
      const v = values[i];
      if (v === null) {
        classes[i] = -1;
        continue;
      }
      let id = ids.get(v);
      if (id === undefined) {
        const next = typeof v === 'string' ? 'string' : 'number';
        if ((kind !== null && kind !== next) || v !== v) return null;
        kind = next;
        id = distinct.length;
        ids.set(v, id);
        distinct.push(v);
      }
      classes[i] = id;
    }
    if (end < n) yield;
  }
  const rankOfId = new Int32Array(distinct.length);
  let sortedDistinct: (string | number)[] = [];
  if (kind === 'string') {
    const sorted = yield* sortDistinctStrings(distinct as string[], chunk);
    for (let r = 0; r < sorted.length; r++) rankOfId[ids.get(sorted[r])!] = r;
    sortedDistinct = sorted;
  } else if (kind === 'number') {
    const sorted = Float64Array.from(distinct as number[]).sort();
    for (let r = 0; r < sorted.length; r++) rankOfId[ids.get(sorted[r])!] = r;
    sortedDistinct = Array.from(sorted);
  }
  const count = distinct.length;
  for (let i = 0; i < n; i++) classes[i] = classes[i] < 0 ? count : rankOfId[classes[i]];
  const result = { classes, count };
  distinctSorted.set(result, sortedDistinct);
  return result;
}

function lowerBound(sorted: readonly (string | number)[], value: string | number): number {
  let low = 0;
  let high = sorted.length;
  while (low < high) {
    const mid = (low + high) >>> 1;
    if (sorted[mid] < value) low = mid + 1;
    else high = mid;
  }
  return low;
}

/**
 * `held` (the classes of `values` before the rows from `from` on were appended) folded with the appended values: the same
 * classes `valueClasses(values)` gives, from sorting only the appended values that are new. `undefined` when that cannot
 * be told cheaply (no distinct values held, a NaN, or values of the other kind); the caller then builds the classes again.
 */
export function extendValueClasses(held: ValueClasses, values: readonly SortValue[], from: number): ValueClasses | undefined {
  const old = distinctSorted.get(held);
  if (!old || old.length === 0 || held.classes.length !== from) return undefined;
  const kind = typeof old[0];
  const n = values.length;
  const fresh = new Set<string | number>();
  const freshValues: (string | number)[] = [];
  for (let i = from; i < n; i++) {
    const v = values[i];
    if (v === null) continue;
    if (typeof v !== kind || v !== v) return undefined;
    if (fresh.has(v)) continue;
    const at = lowerBound(old, v);
    if (old[at] === v) continue;
    fresh.add(v);
    freshValues.push(v);
  }
  const added = kind === 'string' ? (freshValues as string[]).sort() : Array.from(Float64Array.from(freshValues as number[]).sort());
  const count = old.length + added.length;
  const newRankOfOld = new Int32Array(old.length + 1);
  const sorted: (string | number)[] = new Array(count);
  const rankOfFresh = new Map<string | number, number>();
  let previousAt = 0;
  let written = 0;
  for (let j = 0; j <= added.length; j++) {
    const at = j < added.length ? lowerBound(old, added[j]) : old.length;
    for (let r = previousAt; r < at; r++) {
      newRankOfOld[r] = r + j;
      sorted[written++] = old[r];
    }
    if (j < added.length) {
      rankOfFresh.set(added[j], written);
      sorted[written++] = added[j];
    }
    previousAt = at;
  }
  newRankOfOld[old.length] = count;
  const classes = new Int32Array(n);
  for (let i = 0; i < from; i++) classes[i] = newRankOfOld[held.classes[i]];
  for (let i = from; i < n; i++) {
    const v = values[i];
    if (v === null) classes[i] = count;
    else {
      const rank = rankOfFresh.get(v);
      classes[i] = rank !== undefined ? rank : newRankOfOld[lowerBound(old, v)];
    }
  }
  const result = { classes, count };
  distinctSorted.set(result, sorted);
  return result;
}

/** `valueClassesSteps` in one piece. */
export function valueClasses(values: readonly SortValue[]): ValueClasses | null {
  const steps = valueClassesSteps(values, Infinity);
  for (;;) {
    const step = steps.next();
    if (step.done) return step.value;
  }
}

/** The order of `classes` by counting sort: `direction` 1 ascending, -1 descending, no value last, equal values in position order. */
export function orderFromClasses({ classes, count }: ValueClasses, direction: 1 | -1): SortedOrder {
  const n = classes.length;
  const slot = (k: number) => (k === count || direction === 1 ? k : count - 1 - k);
  const start = new Int32Array(count + 2);
  for (let i = 0; i < n; i++) start[slot(classes[i]) + 1]++;
  for (let s = 0; s <= count; s++) start[s + 1] += start[s];
  const order = new Int32Array(n);
  for (let i = 0; i < n; i++) order[start[slot(classes[i])]++] = i;
  return new SortedOrder(order);
}

/**
 * The order `Array.prototype.sort` gives positions compared by value: `direction` 1 ascending, -1 descending, no value
 * last in both, equal values in position order.
 */
export function sortedOrder(values: readonly SortValue[], direction: 1 | -1): SortedOrder {
  const classes = valueClasses(values);
  if (classes) return orderFromClasses(classes, direction);
  const order = Int32Array.from({ length: values.length }, (_, i) => i).sort((a, b) => compareSortValues(values[a], values[b], direction) || a - b);
  return new SortedOrder(order);
}

/** `positions` (any order) sorted by their place in `sorted`; the same as sorting just those positions. */
export function sortedSubset(positions: ArrayLike<number>, sorted: SortedOrder): Int32Array {
  const n = sorted.order.length;
  if (!sorted.rankIsBuilt() && positions.length * 4 > n) {
    const wanted = new Uint8Array(n);
    for (let i = 0; i < positions.length; i++) wanted[positions[i]] = 1;
    const out = new Int32Array(positions.length);
    let count = 0;
    for (let i = 0; i < n && count < out.length; i++) {
      const p = sorted.order[i];
      if (wanted[p] === 1) out[count++] = p;
    }
    return out;
  }
  const keys = new Int32Array(positions.length);
  for (let i = 0; i < positions.length; i++) keys[i] = sorted.rank[positions[i]];
  keys.sort();
  for (let i = 0; i < keys.length; i++) keys[i] = sorted.order[keys[i]];
  return keys;
}

/** Most positions `moveChangedValues` moves one by one. */
export const MAX_MOVED_IN_PLACE = 16;

/**
 * Like `withChangedValues`, in `sorted.order` itself, one binary search and one block move per changed position, so a
 * bump of a few rows costs next to nothing at any list size. `false` (nothing touched) when more than `MAX_MOVED_IN_PLACE`
 * positions changed.
 */
export function moveChangedValues(sorted: SortedOrder, values: readonly SortValue[], changed: readonly number[], direction: 1 | -1): boolean {
  const moving = [...new Set(changed)];
  if (moving.length > MAX_MOVED_IN_PLACE) return false;
  if (moving.length === 0) return true;
  const order = sorted.order;
  let placed = order.length;
  for (const p of moving) {
    const at = order.subarray(0, placed).indexOf(p);
    order.copyWithin(at, at + 1, placed);
    placed--;
  }
  moving.sort((a, b) => compareSortValues(values[a], values[b], direction) || a - b);
  for (const p of moving) {
    const value = values[p];
    let low = 0;
    let high = placed;
    while (low < high) {
      const mid = (low + high) >>> 1;
      const other = order[mid];
      const c = compareSortValues(values[other], value, direction);
      if (c < 0 || (c === 0 && other < p)) low = mid + 1;
      else high = mid;
    }
    order.copyWithin(low + 1, low, placed);
    order[low] = p;
    placed++;
  }
  sorted.orderMoved();
  return true;
}

/** Most positions `withChangedValues` repositions before a new sort is cheaper. */
export const MAX_REPOSITIONED = 4096;

/**
 * `sorted` after the values at `changed` positions were replaced in `values` (already written): the same order a new
 * `sortedOrder(values, direction)` gives, by moving only those positions. `null` when more than `MAX_REPOSITIONED` moved.
 */
export function withChangedValues(sorted: SortedOrder, values: readonly SortValue[], changed: readonly number[], direction: 1 | -1): SortedOrder | null {
  if (changed.length === 0) return sorted;
  const moving = [...new Set(changed)];
  if (moving.length > MAX_REPOSITIONED) return null;
  const n = sorted.order.length;
  const isMoving = new Uint8Array(n);
  for (const p of moving) isMoving[p] = 1;
  const kept = new Int32Array(n - moving.length);
  let keptCount = 0;
  for (let i = 0; i < n; i++) {
    const p = sorted.order[i];
    if (!isMoving[p]) kept[keptCount++] = p;
  }
  const before = (position: number, value: SortValue, other: number) => {
    const c = compareSortValues(values[other], value, direction);
    return c < 0 || (c === 0 && other < position);
  };
  moving.sort((a, b) => compareSortValues(values[a], values[b], direction) || a - b);
  const order = new Int32Array(n);
  let from = 0;
  let written = 0;
  for (const p of moving) {
    let low = from;
    let high = keptCount;
    while (low < high) {
      const mid = (low + high) >>> 1;
      if (before(p, values[p], kept[mid])) low = mid + 1;
      else high = mid;
    }
    order.set(kept.subarray(from, low), written);
    written += low - from;
    from = low;
    order[written++] = p;
  }
  order.set(kept.subarray(from), written);
  return new SortedOrder(order);
}
