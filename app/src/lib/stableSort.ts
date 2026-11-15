export function sameMembers(members: Set<unknown>, rows: readonly unknown[]): boolean {
  if (rows.length !== members.size) return false;
  for (const row of rows) if (!members.has(row)) return false;
  return true;
}

export interface RowRange {
  first: number;
  end: number;
}

/**
 * Rows to scroll by after `before` was re-sorted into `after`, so the rows in `visible` stay where they are on screen:
 * the shift most of them share (rows that moved on their own, such as a track whose BPM just arrived, are outvoted).
 */
export function anchorShift<T>(before: readonly T[], after: readonly T[], visible: RowRange | null): number {
  if (!visible) return 0;
  const first = Math.max(0, visible.first);
  const end = Math.min(before.length, visible.end);
  if (end <= first) return 0;
  const oldIndex = new Map<T, number>();
  for (let i = first; i < end; i++) oldIndex.set(before[i], i);
  const votes = new Map<number, number>();
  let found = 0;
  for (let i = 0; i < after.length && found < oldIndex.size; i++) {
    const was = oldIndex.get(after[i]);
    if (was === undefined) continue;
    found++;
    votes.set(i - was, (votes.get(i - was) ?? 0) + 1);
  }
  let best = 0;
  let bestVotes = 0;
  for (const [shift, count] of votes) {
    if (count > bestVotes || (count === bestVotes && Math.abs(shift) < Math.abs(best))) {
      best = shift;
      bestVotes = count;
    }
  }
  return best;
}
