import { beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.hoisted(() => vi.fn());
const saveMock = vi.hoisted(() => vi.fn());
const clipboard = vi.hoisted(() => ({ writeText: vi.fn(async () => {}) }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), save: saveMock }));

async function fresh() {
  vi.resetModules();
  vi.stubGlobal('window', { __TAURI_INTERNALS__: {} });
  vi.stubGlobal('navigator', { clipboard });
  const library = await import('./library.svelte.ts');
  const exporter = await import('./playlist-export.svelte.ts');
  return { library: library.library, ...exporter };
}

function backend(reply: Record<string, unknown>) {
  invokeMock.mockImplementation(async (command: string) => {
    if (command in reply) {
      const value = reply[command];
      if (value instanceof Error) throw value;
      return value;
    }
    return undefined;
  });
}

beforeEach(() => {
  invokeMock.mockReset();
  saveMock.mockReset();
  clipboard.writeText.mockClear();
});

describe('exportPlaylistToFile', () => {
  it('offers the dated name, exports to the chosen path and reports the counts', async () => {
    backend({
      playlist_export_file_name: 'Warmup 2026-11-21.m3u8',
      export_playlist: { written: 4, missing: 1, missing_examples: ['/gone.mp3'] },
    });
    saveMock.mockResolvedValue('/out/Warmup 2026-11-21.m3u8');
    const { library, exportPlaylistToFile } = await fresh();

    await exportPlaylistToFile(2, 'm3u8');

    expect(saveMock).toHaveBeenCalledWith({
      defaultPath: 'Warmup 2026-11-21.m3u8',
      filters: [{ name: 'M3U8 playlist', extensions: ['m3u8'] }],
    });
    expect(invokeMock).toHaveBeenCalledWith('export_playlist', { playlistId: 2, format: 'm3u8', destPath: '/out/Warmup 2026-11-21.m3u8' });
    expect(library.statusMessage).toBe('Exported 4 tracks to Warmup 2026-11-21.m3u8 · 1 audio file not found, still listed');
  });

  it('does nothing when the save dialog is cancelled', async () => {
    backend({ playlist_export_file_name: 'x.csv' });
    saveMock.mockResolvedValue(null);
    const { library, exportPlaylistToFile } = await fresh();
    const before = library.statusMessage;

    await exportPlaylistToFile(1, 'csv');

    expect(invokeMock.mock.calls.map((c) => c[0])).toEqual(['playlist_export_file_name']);
    expect(library.statusMessage).toBe(before);
  });

  it('shows a plain message when the export fails', async () => {
    backend({ playlist_export_file_name: 'x.csv', export_playlist: new Error('Failed to write /out/x.csv') });
    saveMock.mockResolvedValue('/out/x.csv');
    const { library, exportPlaylistToFile } = await fresh();

    await exportPlaylistToFile(1, 'csv');

    expect(library.statusMessage.startsWith("Couldn't export the playlist.")).toBe(true);
  });
});

describe('copyPlaylistTracklist', () => {
  it('puts the backend tracklist on the clipboard and counts its lines', async () => {
    backend({ playlist_tracklist: 'A - One\nB - Two\n' });
    const { library, copyPlaylistTracklist } = await fresh();

    await copyPlaylistTracklist(1);

    expect(clipboard.writeText).toHaveBeenCalledWith('A - One\nB - Two\n');
    expect(library.statusMessage).toBe('Copied a tracklist of 2 tracks');
  });
});
