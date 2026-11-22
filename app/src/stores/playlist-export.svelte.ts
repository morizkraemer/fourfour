import { library } from './library.svelte.ts';
import {
  exportPlaylist,
  pickExportDestination,
  playlistExportFileName,
  playlistTracklist,
} from '../services/tauri.svelte.ts';
import { copiedSummary, exportFormatInfo, exportSummary, type ExportFormat } from '../lib/playlistExport.ts';
import { copyTracklistFailureText, playlistExportFailureText } from '../lib/userErrors.ts';

export async function exportPlaylistToFile(playlistId: number, format: ExportFormat): Promise<void> {
  const info = exportFormatInfo(format);
  try {
    const defaultName = await playlistExportFileName(playlistId, format);
    const destPath = await pickExportDestination(defaultName, info.extension, info.filterName);
    if (!destPath) return;
    const report = await exportPlaylist(playlistId, format, destPath);
    library.statusMessage = exportSummary(report, destPath);
  } catch (err) {
    console.error('exportPlaylist failed:', err);
    library.statusMessage = playlistExportFailureText(err);
  }
}

async function writeClipboard(text: Promise<string>): Promise<void> {
  if (typeof ClipboardItem !== 'undefined' && navigator.clipboard?.write) {
    const blob = text.then((value) => new Blob([value], { type: 'text/plain' }));
    await navigator.clipboard.write([new ClipboardItem({ 'text/plain': blob })]);
    return;
  }
  await navigator.clipboard.writeText(await text);
}

export async function copyPlaylistTracklist(playlistId: number): Promise<void> {
  try {
    const text = playlistTracklist(playlistId);
    await writeClipboard(text);
    const lines = (await text).split('\n').filter((line) => line.length > 0).length;
    library.statusMessage = copiedSummary(lines);
  } catch (err) {
    console.error('copyPlaylistTracklist failed:', err);
    library.statusMessage = copyTracklistFailureText(err);
  }
}
