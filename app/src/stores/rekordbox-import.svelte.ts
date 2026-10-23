import {
  listenToRekordboxImportProgress,
  pickDirectory,
  pickFile,
  rekordboxImportPreview,
  rekordboxImportRun,
  type RekordboxImportPreview,
  type RekordboxImportProgress,
  type RekordboxImportReport,
  type RekordboxSourceKind,
} from '../services/tauri.svelte.ts';
import { loadState } from './library.svelte.ts';

export type RekordboxImportPhase = 'idle' | 'scanning' | 'ready' | 'importing' | 'done' | 'error';

export const REKORDBOX_SOURCES: { kind: RekordboxSourceKind; label: string }[] = [
  { kind: 'master_db', label: 'Rekordbox' },
  { kind: 'usb', label: 'USB stick' },
  { kind: 'xml', label: 'XML' },
];

export const rekordboxImport = $state({
  open: false,
  kind: 'master_db' as RekordboxSourceKind,
  path: '',
  prefix: '',
  musicRoot: '',
  includeMissing: false,
  phase: 'idle' as RekordboxImportPhase,
  preview: null as RekordboxImportPreview | null,
  report: null as RekordboxImportReport | null,
  progress: null as RekordboxImportProgress | null,
  error: '',
});

function request() {
  return {
    kind: rekordboxImport.kind,
    path: rekordboxImport.path,
    remap_from: rekordboxImport.prefix || null,
    remap_to: rekordboxImport.musicRoot || null,
    include_missing: rekordboxImport.includeMissing,
  };
}

function resetResults() {
  rekordboxImport.phase = 'idle';
  rekordboxImport.preview = null;
  rekordboxImport.report = null;
  rekordboxImport.progress = null;
  rekordboxImport.error = '';
}

export function openRekordboxImport() {
  resetResults();
  rekordboxImport.open = true;
}

export function closeRekordboxImport() {
  if (rekordboxImport.phase === 'importing' || rekordboxImport.phase === 'scanning') return;
  rekordboxImport.open = false;
}

export function setRekordboxKind(kind: RekordboxSourceKind) {
  rekordboxImport.kind = kind;
  rekordboxImport.path = '';
  rekordboxImport.prefix = '';
  resetResults();
}

export function setRekordboxIncludeMissing(include: boolean) {
  rekordboxImport.includeMissing = include;
  if (rekordboxImport.preview) void scanRekordboxSource();
}

export async function chooseRekordboxSource() {
  const picked =
    rekordboxImport.kind === 'usb'
      ? await pickDirectory()
      : rekordboxImport.kind === 'xml'
        ? await pickFile({ name: 'Rekordbox XML', extensions: ['xml'] })
        : await pickFile({ name: 'Rekordbox master.db', extensions: ['db'] });
  if (!picked) return;
  rekordboxImport.path = picked;
  rekordboxImport.prefix = '';
  await scanRekordboxSource();
}

export async function chooseRekordboxMusicRoot() {
  const picked = await pickDirectory();
  if (!picked) return;
  rekordboxImport.musicRoot = picked;
  if (rekordboxImport.path) await scanRekordboxSource();
}

export async function scanRekordboxSource() {
  if (!rekordboxImport.path) return;
  rekordboxImport.phase = 'scanning';
  rekordboxImport.error = '';
  try {
    rekordboxImport.preview = await rekordboxImportPreview(request());
    if (!rekordboxImport.prefix) rekordboxImport.prefix = rekordboxImport.preview.detected_root;
    rekordboxImport.phase = 'ready';
  } catch (err) {
    rekordboxImport.preview = null;
    rekordboxImport.error = String(err);
    rekordboxImport.phase = 'error';
  }
}

export async function runRekordboxImport() {
  if (rekordboxImport.phase !== 'ready') return;
  rekordboxImport.phase = 'importing';
  rekordboxImport.progress = null;
  const stopListening = listenToRekordboxImportProgress((p) => {
    rekordboxImport.progress = p;
  });
  try {
    rekordboxImport.report = await rekordboxImportRun(request());
    await loadState();
    rekordboxImport.phase = 'done';
  } catch (err) {
    rekordboxImport.error = String(err);
    rekordboxImport.phase = 'error';
  } finally {
    stopListening();
  }
}
