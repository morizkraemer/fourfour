import {
  listenToUsbSyncProgress,
  type PlanTrack,
  planUsbSync,
  syncUsbMany,
  type StickProgress,
  type StickReport,
  type SyncTarget,
  type UsbSyncPlan,
} from '../services/tauri.svelte.ts';
import { loadCapacity, sticks } from './sticks.svelte.ts';
import { syncFailureText } from '../lib/userErrors.ts';
import { stickSelection } from '../lib/stickEdits.ts';
import { planChanges, stickStatus, type StickStatus } from '../lib/stickStatus.ts';

export type StickSyncStage = 'idle' | 'planning' | 'confirm' | 'writing' | 'done' | 'failed';

export const stickSync = $state({
  stage: 'idle' as StickSyncStage,
  volumePath: null as string | null,
  plan: null as UsbSyncPlan | null,
  progress: null as StickProgress | null,
  report: null as StickReport | null,
  error: '',
});

export function stickSyncTarget(volumePath: string): SyncTarget {
  return { volume: volumePath, selection: stickSelection(sticks.byPath[volumePath]) };
}

export interface StickPendingPlan {
  changes: number;
  adds: number;
  removes: number;
  fits: boolean | null;
  bytesNeeded: number;
  freeBytes: number | null;
  skipped: PlanTrack[];
  unanalysed: number;
}

export const stickProblemView = $state({ volumePath: null as string | null, anchor: null as HTMLElement | null });

export function toggleStickProblem(volumePath: string, anchor: HTMLElement) {
  if (stickProblemView.volumePath === volumePath && stickProblemView.anchor === anchor) {
    closeStickProblem();
    return;
  }
  stickProblemView.volumePath = volumePath;
  stickProblemView.anchor = anchor;
}

export function closeStickProblem() {
  stickProblemView.volumePath = null;
  stickProblemView.anchor = null;
}

export const stickPending = $state({ byPath: {} as Record<string, StickPendingPlan> });

const PENDING_CHECK_DELAY_MS = 700;
const watchedSignatures = new Map<string, string>();
const pendingTimers = new Map<string, ReturnType<typeof setTimeout>>();
const pendingRuns = new Map<string, number>();

function rememberPlan(volumePath: string, plan: UsbSyncPlan) {
  stickPending.byPath[volumePath] = {
    changes: planChanges(plan),
    adds: plan.add.length,
    removes: plan.remove.length,
    fits: plan.fits,
    bytesNeeded: plan.bytes_needed,
    freeBytes: plan.free_bytes,
    skipped: plan.skipped_unanalyzed,
    unanalysed: plan.skipped_unanalyzed.length,
  };
}

export async function checkStickPending(volumePath: string): Promise<void> {
  if (isStickSyncBusy() || stickSync.stage === 'confirm') return;
  const run = (pendingRuns.get(volumePath) ?? 0) + 1;
  pendingRuns.set(volumePath, run);
  try {
    const plan = await planUsbSync(stickSyncTarget(volumePath));
    if (pendingRuns.get(volumePath) === run && watchedSignatures.has(volumePath)) rememberPlan(volumePath, plan);
  } catch (err) {
    console.info('pending check failed:', err);
    if (pendingRuns.get(volumePath) === run) delete stickPending.byPath[volumePath];
  }
}

export function watchStick(volumePath: string, signature: string) {
  if (watchedSignatures.get(volumePath) === signature) return;
  watchedSignatures.set(volumePath, signature);
  clearTimeout(pendingTimers.get(volumePath));
  pendingTimers.set(volumePath, setTimeout(() => void checkStickPending(volumePath), PENDING_CHECK_DELAY_MS));
}

export function unwatchSticksExcept(volumePaths: string[]) {
  for (const path of [...watchedSignatures.keys()]) {
    if (volumePaths.includes(path)) continue;
    watchedSignatures.delete(path);
    clearTimeout(pendingTimers.get(path));
    pendingTimers.delete(path);
    delete stickPending.byPath[path];
  }
}

export function statusOfStick(volumePath: string): StickStatus {
  const plan = stickPending.byPath[volumePath];
  const edited = sticks.editedSinceSync[volumePath] === true;
  return stickStatus(volumePath, stickSync, plan ? { ...plan, edited } : edited ? { changes: 0, fits: null, edited } : null);
}

export function planNeedsConfirm(plan: UsbSyncPlan): boolean {
  return (
    plan.remove.length > 0 ||
    plan.foreign_kept_count > 0 ||
    plan.fits === false ||
    plan.skipped_unanalyzed.length > 0
  );
}

export function isStickSyncBusy(): boolean {
  return stickSync.stage === 'planning' || stickSync.stage === 'writing';
}

let onFinished: ((volumePath: string) => Promise<void> | void) | null = null;

export function onStickSyncFinished(callback: (volumePath: string) => Promise<void> | void) {
  onFinished = callback;
}

let stopListening: (() => void) | null = null;

function listenOnce() {
  if (stopListening) return;
  stopListening = listenToUsbSyncProgress((progress) => {
    if (stickSync.stage !== 'writing') return;
    stickSync.progress = progress;
  });
}

export async function syncStick(volumePath: string): Promise<void> {
  if (isStickSyncBusy() || stickSync.stage === 'confirm') return;
  listenOnce();
  stickSync.stage = 'planning';
  stickSync.volumePath = volumePath;
  stickSync.plan = null;
  stickSync.progress = null;
  stickSync.report = null;
  stickSync.error = '';
  try {
    const plan = await planUsbSync(stickSyncTarget(volumePath));
    stickSync.plan = plan;
    if (watchedSignatures.has(volumePath)) rememberPlan(volumePath, plan);
    if (planNeedsConfirm(plan)) {
      stickSync.stage = 'confirm';
      return;
    }
  } catch (err) {
    console.info('plan_usb_sync failed:', err);
    stickSync.stage = 'failed';
    stickSync.error = syncFailureText(err);
    return;
  }
  await writeStick(volumePath);
}

export async function confirmStickSync(): Promise<void> {
  const { stage, volumePath, plan } = stickSync;
  if (stage !== 'confirm' || !volumePath || plan?.fits === false) return;
  await writeStick(volumePath);
}

export function cancelStickSync() {
  if (stickSync.stage !== 'confirm') return;
  stickSync.stage = 'idle';
  stickSync.plan = null;
}

export function dismissStickSync() {
  if (isStickSyncBusy()) return;
  stickSync.stage = 'idle';
  stickSync.volumePath = null;
  stickSync.plan = null;
  stickSync.progress = null;
  stickSync.report = null;
  stickSync.error = '';
}

async function writeStick(volumePath: string) {
  stickSync.stage = 'writing';
  stickSync.progress = null;
  try {
    const [report] = await syncUsbMany([stickSyncTarget(volumePath)]);
    stickSync.report = report ?? null;
    if (!report || report.status === 'failed') {
      stickSync.stage = 'failed';
      if (report?.error) console.info('sync failed:', report.error);
      stickSync.error = syncFailureText(report?.error ?? 'The sync did not report a result.', report?.error_kind ?? null);
    } else {
      stickSync.stage = 'done';
      delete sticks.editedSinceSync[volumePath];
    }
  } catch (err) {
    console.info('sync_usb_many failed:', err);
    stickSync.stage = 'failed';
    stickSync.error = syncFailureText(err);
  }
  await loadCapacity(volumePath);
  await onFinished?.(volumePath);
  if (watchedSignatures.has(volumePath)) await checkStickPending(volumePath);
}

const MB = 1_000_000;

export function formatBytes(bytes: number): string {
  if (bytes >= 1000 * MB) return `${(bytes / (1000 * MB)).toFixed(1)} GB`;
  if (bytes >= MB) return `${(bytes / MB).toFixed(1)} MB`;
  return `${Math.max(0, Math.round(bytes / 1000))} KB`;
}

export function syncStatusLine(): string {
  const { stage, progress, report, error } = stickSync;
  if (stage === 'planning') return 'Checking what changes…';
  if (stage === 'writing') {
    if (!progress) return 'Starting…';
    return progress.total > 0 ? `${progress.message} · ${progress.done}/${progress.total}` : progress.message;
  }
  if (stage === 'done' && report?.report) {
    const r = report.report;
    const parts = [`${r.tracks_added} added`];
    if (r.tracks_removed > 0) parts.push(`${r.tracks_removed} removed`);
    if (r.tracks_updated + r.tracks_replaced > 0) parts.push(`${r.tracks_updated + r.tracks_replaced} updated`);
    return `Synced · ${parts.join(' · ')} · Safe to eject`;
  }
  if (stage === 'failed') return error || 'The sync stopped. Check the stick, then sync again.';
  return '';
}
