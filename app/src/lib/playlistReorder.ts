export type MoveDirection = 'up' | 'down';

/** The order with every selected id moved one row up or down; selected blocks keep their shape and stop at the ends. Null when nothing can move. */
export function moveSelectedByOne(order: number[], selected: ReadonlySet<number>, direction: MoveDirection): number[] | null {
  const next = [...order];
  let moved = false;
  if (direction === 'up') {
    for (let i = 1; i < next.length; i++) {
      if (selected.has(next[i]) && !selected.has(next[i - 1])) {
        [next[i - 1], next[i]] = [next[i], next[i - 1]];
        moved = true;
      }
    }
  } else {
    for (let i = next.length - 2; i >= 0; i--) {
      if (selected.has(next[i]) && !selected.has(next[i + 1])) {
        [next[i + 1], next[i]] = [next[i], next[i + 1]];
        moved = true;
      }
    }
  }
  return moved ? next : null;
}
