import { flushSync } from 'svelte';

export function watchValues<T>(read: () => T): { values: T[]; stop: () => void } {
  const values: T[] = [];
  const stop = $effect.root(() => {
    $effect.pre(() => {
      values.push(read());
    });
  });
  flushSync();
  return { values, stop };
}

export function heldInState<T>(value: T): T {
  const box = $state({ value });
  return box.value;
}
