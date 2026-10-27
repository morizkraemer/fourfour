import { fuzzyMatch, fuzzyRank } from './fuzzy.ts';

export interface PaletteEntry {
  id: string;
  name: string;
  sub?: string;
  right?: string;
  icon: string;
  ranges?: number[];
  control?: PaletteControl;
  keepOpen?: boolean;
  choose?: (value: string) => void;
  run: () => void;
}

export type PaletteControl =
  | { kind: 'switch'; on: boolean }
  | { kind: 'choice'; value: string; options: { value: string; label: string }[] };

export interface PaletteSettingSource {
  id: string;
  label: string;
  control?: PaletteControl;
  change: (value?: string) => void;
}

export interface PaletteGroupModel {
  key: 'actions' | 'settings' | 'places' | 'tracks';
  title: string;
  items: PaletteEntry[];
}

export interface PaletteSources {
  actions: { id: string; label: string; shortcut: string; run: () => void }[];
  settings?: PaletteSettingSource[];
  places: { id: string; label: string; sub?: string; icon: string; run: () => void }[];
  tracks: { id: number; title: string; artist: string; run: () => void }[];
}

export const EMPTY_QUERY_LIMITS = { actions: 12, settings: 0, places: 6, tracks: 0 };
export const QUERY_LIMITS = { actions: 8, settings: 8, places: 8, tracks: 8 };

export function buildPaletteGroups(query: string, sources: PaletteSources): PaletteGroupModel[] {
  const q = query.trim();
  const limits = q ? QUERY_LIMITS : EMPTY_QUERY_LIMITS;

  const actions = fuzzyRank(q, sources.actions, (a) => a.label)
    .slice(0, limits.actions)
    .map(({ item, match }): PaletteEntry => ({
      id: `action:${item.id}`,
      name: item.label,
      right: item.shortcut,
      icon: 'chevron-right',
      ranges: match.indices,
      run: item.run,
    }));

  const settings = fuzzyRank(q, sources.settings ?? [], (r) => r.label)
    .slice(0, limits.settings)
    .map(({ item, match }): PaletteEntry => ({
      id: `setting:${item.id}`,
      name: item.label,
      icon: item.control ? 'settings' : 'chevron-right',
      ranges: match.indices,
      control: item.control,
      keepOpen: item.control !== undefined,
      choose: (value) => item.change(value),
      run: () => item.change(),
    }));

  const places = fuzzyRank(q, sources.places, (p) => p.label)
    .slice(0, limits.places)
    .map(({ item, match }): PaletteEntry => ({
      id: `place:${item.id}`,
      name: item.label,
      sub: item.sub,
      icon: item.icon,
      ranges: match.indices,
      run: item.run,
    }));

  const tracks = q
    ? fuzzyRank(q, sources.tracks, (t) => `${t.title} ${t.artist}`)
        .slice(0, limits.tracks)
        .map(({ item }): PaletteEntry => ({
          id: `track:${item.id}`,
          name: item.title,
          sub: item.artist,
          icon: 'list',
          ranges: fuzzyMatch(q, item.title)?.indices,
          run: item.run,
        }))
    : [];

  return [
    { key: 'actions', title: 'Actions', items: actions },
    { key: 'settings', title: 'Settings', items: settings },
    { key: 'places', title: 'Go to', items: places },
    { key: 'tracks', title: 'Tracks', items: tracks },
  ];
}
