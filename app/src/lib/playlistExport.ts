import { pathBaseName } from './pathName.ts';

export type ExportFormat = 'm3u8' | 'csv' | 'text' | 'traktor_nml' | 'rekordbox_xml';

export interface ExportReport {
  written: number;
  missing: number;
  missing_examples: string[];
}

export interface ExportFormatInfo {
  format: ExportFormat;
  label: string;
  extension: string;
  filterName: string;
  foldersToo: boolean;
}

export const EXPORT_FORMATS: readonly ExportFormatInfo[] = [
  { format: 'm3u8', label: 'M3U8 playlist', extension: 'm3u8', filterName: 'M3U8 playlist', foldersToo: false },
  { format: 'csv', label: 'CSV spreadsheet', extension: 'csv', filterName: 'CSV', foldersToo: false },
  { format: 'text', label: 'Text tracklist', extension: 'txt', filterName: 'Text', foldersToo: false },
  { format: 'traktor_nml', label: 'Traktor (NML)', extension: 'nml', filterName: 'Traktor collection', foldersToo: true },
  { format: 'rekordbox_xml', label: 'Rekordbox XML', extension: 'xml', filterName: 'Rekordbox XML', foldersToo: true },
];

export function exportFormatInfo(format: ExportFormat): ExportFormatInfo {
  return EXPORT_FORMATS.find((f) => f.format === format)!;
}

function countText(count: number, noun: string): string {
  return `${count} ${noun}${count === 1 ? '' : 's'}`;
}

export function exportSummary(report: ExportReport, destPath: string): string {
  const file = pathBaseName(destPath);
  if (report.written === 0) return `Exported an empty list to ${file}`;
  const base = `Exported ${countText(report.written, 'track')} to ${file}`;
  return report.missing > 0 ? `${base} · ${countText(report.missing, 'audio file')} not found, still listed` : base;
}

export function copiedSummary(trackCount: number): string {
  return trackCount === 0 ? 'Nothing to copy: the playlist is empty' : `Copied a tracklist of ${countText(trackCount, 'track')}`;
}
