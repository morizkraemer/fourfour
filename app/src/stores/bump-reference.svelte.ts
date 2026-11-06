export function snapshotOf<T>(value: T): T {
  return $state.snapshot(value) as T;
}
