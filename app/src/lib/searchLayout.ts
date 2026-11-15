export type SearchMode = 'field' | 'icon' | 'sidebar';

export const SEARCH_FIELD_MIN_COLUMN = 720;
export const SEARCH_ICON_MIN_COLUMN = 440;

export function searchModeForColumn(columnWidth: number): SearchMode {
  if (columnWidth >= SEARCH_FIELD_MIN_COLUMN) return 'field';
  if (columnWidth >= SEARCH_ICON_MIN_COLUMN) return 'icon';
  return 'sidebar';
}
