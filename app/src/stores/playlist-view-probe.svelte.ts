import { filterTracks, tracksForSidebarSource } from './library.svelte.ts';
import { resolveLayoutKind, sortTracks } from './table-columns.svelte.ts';

/** The row chain of `TrackTable` (source rows, search filter, sort) as deriveds, so tests read rows the way the table does. */
export function openTableView(first: string) {
  const state = $state({ source: first, query: '' });
  const base = $derived.by(() => tracksForSidebarSource(state.source));
  const filtered = $derived(filterTracks(base, state.query));
  const kind = $derived(resolveLayoutKind(state.source));
  const rows = $derived(sortTracks(filtered, kind));
  return {
    get rows() {
      return rows;
    },
    get kind() {
      return kind;
    },
    show(source: string) {
      state.source = source;
    },
    search(query: string) {
      state.query = query;
    },
  };
}
