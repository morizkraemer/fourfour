import { describe, expect, it } from 'vitest';
import { filterTracks } from './library.svelte.ts';

const track = (id: number, over: Record<string, unknown> = {}) => ({
  id,
  title: `Track ${id}`,
  artist: 'Artist',
  album: '',
  bpm: '120.0',
  key: '8A',
  raw: { source_path: `/music/t${id}.mp3` },
  ...over,
});

describe('filterTracks search-text cache', () => {
  it('returns the same array for an empty or whitespace query', () => {
    const tracks = [track(1)];
    expect(filterTracks(tracks, '')).toBe(tracks);
    expect(filterTracks(tracks, '   ')).toBe(tracks);
  });
  it('matches case-insensitively across fields', () => {
    const tracks = [track(1, { title: 'Deep House' }), track(2, { artist: 'Moby' })];
    expect(filterTracks(tracks, 'DEEP').map((t) => t.id)).toEqual([1]);
    expect(filterTracks(tracks, 'moby').map((t) => t.id)).toEqual([2]);
  });
  it('requires every whitespace-separated term to match', () => {
    const tracks = [track(1, { title: 'Alpha', key: '8A' }), track(2, { title: 'Alpha', key: '9B' })];
    expect(filterTracks(tracks, 'alpha 9b').map((t) => t.id)).toEqual([2]);
    expect(filterTracks(tracks, 'alpha zzz')).toEqual([]);
  });
  it('searches raw backend fields such as the source path', () => {
    const tracks = [track(1), track(2)];
    expect(filterTracks(tracks, 't2.mp3').map((t) => t.id)).toEqual([2]);
  });
  it('treats missing fields as empty text and not as "undefined"', () => {
    const tracks = [track(1, { album: undefined, label: null })];
    expect(filterTracks(tracks, 'undefined')).toEqual([]);
    expect(filterTracks(tracks, 'null')).toEqual([]);
  });
  it('reuses cached text for the same array even if a track mutates', () => {
    const tracks = [track(1, { title: 'Before' })];
    expect(filterTracks(tracks, 'before')).toHaveLength(1);
    tracks[0].title = 'After';
    expect(filterTracks(tracks, 'before')).toHaveLength(1);
    expect(filterTracks(tracks, 'after')).toHaveLength(0);
  });
  it('rebuilds text once the array is swapped, as bumpLibraryTracks does', () => {
    const tracks = [track(1, { title: 'Before' })];
    filterTracks(tracks, 'before');
    tracks[0].title = 'After';
    const swapped = [...tracks];
    expect(filterTracks(swapped, 'after')).toHaveLength(1);
    expect(filterTracks(swapped, 'before')).toHaveLength(0);
  });
  it('keeps result order and identity of matching rows', () => {
    const tracks = [track(1), track(2), track(3)];
    const out = filterTracks(tracks, 'track');
    expect(out).toEqual(tracks);
    expect(out[1]).toBe(tracks[1]);
  });
});
