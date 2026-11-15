/*
 * Svelte's `$state` wraps every array whose prototype is `Array.prototype` in a proxy that makes a signal per element
 * it reads: a 20,000-id playlist cost 100-200 ms to read once and 20,000 signals to keep. An array with another
 * prototype object is left alone, so the track ids of a playlist are marked with one: such an array is replaced
 * as a whole when the playlist changes (never changed in place), and `track_ids` is the one signal that follows it.
 */
const rawIdsPrototype: object = Object.create(Array.prototype);

/** Marks a fresh array of track ids (not one a caller keeps) as raw state; returns it. */
export function rawIds(ids: number[]): number[] {
  return Object.setPrototypeOf(ids, rawIdsPrototype);
}

export function isRawIds(ids: readonly number[]): boolean {
  return Object.getPrototypeOf(ids) === rawIdsPrototype;
}
