import { describe, expect, it } from 'vitest';
import { EXPORT_FORMATS, copiedSummary, exportFormatInfo, exportSummary } from './playlistExport.ts';

describe('exportSummary', () => {
  it('names the track count and the file', () => {
    expect(exportSummary({ written: 12, missing: 0, missing_examples: [] }, '/Users/me/Set 2026-11-21.m3u8')).toBe(
      'Exported 12 tracks to Set 2026-11-21.m3u8'
    );
    expect(exportSummary({ written: 1, missing: 0, missing_examples: [] }, 'C:\\out\\a.csv')).toBe('Exported 1 track to a.csv');
  });
  it('reports tracks whose audio file is missing as still listed', () => {
    expect(exportSummary({ written: 5, missing: 2, missing_examples: ['/a'] }, '/o/x.nml')).toBe(
      'Exported 5 tracks to x.nml · 2 audio files not found, still listed'
    );
    expect(exportSummary({ written: 5, missing: 1, missing_examples: ['/a'] }, '/o/x.nml')).toContain('1 audio file not found');
  });
  it('says so when the playlist was empty', () => {
    expect(exportSummary({ written: 0, missing: 0, missing_examples: [] }, '/o/x.txt')).toBe('Exported an empty list to x.txt');
  });
});

describe('copiedSummary', () => {
  it('counts tracks and handles an empty playlist', () => {
    expect(copiedSummary(3)).toBe('Copied a tracklist of 3 tracks');
    expect(copiedSummary(1)).toBe('Copied a tracklist of 1 track');
    expect(copiedSummary(0)).toBe('Nothing to copy: the playlist is empty');
  });
});

describe('EXPORT_FORMATS', () => {
  it('offers the five formats, folders only as Traktor and Rekordbox', () => {
    expect(EXPORT_FORMATS.map((f) => f.format)).toEqual(['m3u8', 'csv', 'text', 'traktor_nml', 'rekordbox_xml']);
    expect(EXPORT_FORMATS.filter((f) => f.foldersToo).map((f) => f.format)).toEqual(['traktor_nml', 'rekordbox_xml']);
    expect(exportFormatInfo('text').extension).toBe('txt');
  });
});
