import {
  listenToRekordboxImportProgress,
  pickDirectory,
  pickFile,
  rekordboxDefaultMasterDb,
  rekordboxImportPreview,
  rekordboxImportRun,
  type RekordboxImportPreview,
  type RekordboxImportProgress,
  type RekordboxImportReport,
  type RekordboxRemapStat,
  type RekordboxSourceKind,
} from '../services/tauri.svelte.ts';
import { loadState } from './library.svelte.ts';
import { rekordboxFailureText } from '../lib/userErrors.ts';

export type RekordboxImportPhase = 'idle' | 'scanning' | 'ready' | 'importing' | 'done' | 'error';

export type RekordboxRemapRow = { from: string; to: string };

export const REKORDBOX_SOURCES: { kind: RekordboxSourceKind; label: string }[] = [
  { kind: 'master_db', label: 'Rekordbox' },
  { kind: 'usb', label: 'USB stick' },
  { kind: 'xml', label: 'XML' },
];

export const rekordboxImport = $state({
  open: false,
  kind: 'master_db' as RekordboxSourceKind,
  path: '',
  rows: [] as RekordboxRemapRow[],
  includeMissing: false,
  phase: 'idle' as RekordboxImportPhase,
  preview: null as RekordboxImportPreview | null,
  report: null as RekordboxImportReport | null,
  progress: null as RekordboxImportProgress | null,
  error: '',
});

function activeRows() {
  return rekordboxImport.rows.filter((row) => row.to);
}

function request() {
  return {
    kind: rekordboxImport.kind,
    path: rekordboxImport.path,
    remaps: activeRows().map((row) => ({ from_prefix: row.from, to_prefix: row.to })),
    include_missing: rekordboxImport.includeMissing,
  };
}

/** Preview result for each row, in row order; rows without a target have none yet. */
export function rowStats(): (RekordboxRemapStat | null)[] {
  const stats = rekordboxImport.preview?.remap_stats ?? [];
  let next = 0;
  return rekordboxImport.rows.map((row) => (row.to ? (stats[next++] ?? null) : null));
}

function resetResults() {
  rekordboxImport.phase = 'idle';
  rekordboxImport.preview = null;
  rekordboxImport.report = null;
  rekordboxImport.progress = null;
  rekordboxImport.error = '';
}

async function useDefaultMasterDb() {
  if (rekordboxImport.kind !== 'master_db' || rekordboxImport.path) return;
  const found = await rekordboxDefaultMasterDb();
  if (!found || rekordboxImport.path || rekordboxImport.kind !== 'master_db') return;
  rekordboxImport.path = found;
  await scanRekordboxSource();
}

export async function openRekordboxImport() {
  resetResults();
  rekordboxImport.open = true;
  await useDefaultMasterDb();
}

export function closeRekordboxImport() {
  if (rekordboxImport.phase === 'importing' || rekordboxImport.phase === 'scanning') return;
  rekordboxImport.open = false;
}

export function setRekordboxKind(kind: RekordboxSourceKind) {
  rekordboxImport.kind = kind;
  rekordboxImport.path = '';
  rekordboxImport.rows = [];
  resetResults();
  void useDefaultMasterDb();
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
  rekordboxImport.rows = [];
  await scanRekordboxSource();
}

export function addRekordboxRemapRow() {
  rekordboxImport.rows.push({ from: '', to: '' });
}

export async function removeRekordboxRemapRow(index: number) {
  const hadTarget = !!rekordboxImport.rows[index]?.to;
  rekordboxImport.rows.splice(index, 1);
  if (hadTarget) await scanRekordboxSource();
}

export async function chooseRekordboxRemapTarget(index: number) {
  const picked = await pickDirectory();
  if (!picked || !rekordboxImport.rows[index]) return;
  rekordboxImport.rows[index].to = picked;
  await scanRekordboxSource();
}

export async function rescanRekordboxRemap() {
  if (rekordboxImport.rows.some((row) => row.to)) await scanRekordboxSource();
}

export async function scanRekordboxSource() {
  if (!rekordboxImport.path) return;
  rekordboxImport.phase = 'scanning';
  rekordboxImport.error = '';
  try {
    const preview = await rekordboxImportPreview(request());
    rekordboxImport.preview = preview;
    if (rekordboxImport.rows.length === 0 && preview.detected_root) {
      rekordboxImport.rows.push({ from: preview.detected_root, to: '' });
    }
    rekordboxImport.phase = 'ready';
  } catch (err) {
    rekordboxImport.preview = null;
    console.info('rekordbox preview failed:', err);
    rekordboxImport.error = rekordboxFailureText(err);
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
    console.info('rekordbox import failed:', err);
    rekordboxImport.error = rekordboxFailureText(err, "Couldn't import the Rekordbox library.");
    rekordboxImport.phase = 'error';
  } finally {
    stopListening();
  }
}
