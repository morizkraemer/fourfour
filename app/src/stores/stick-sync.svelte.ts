import {
  listenToUsbSyncProgress,
  planUsbSync,
  syncUsbMany,
  type StickProgress,
  type StickReport,
  type SyncTarget,
  type UsbSyncPlan,
} from '../services/tauri.svelte.ts';
import { loadCapacity, stickAllTrackIds, stickPlaylistIds } from './sticks.svelte.ts';
import { syncFailureText } from '../lib/userErrors.ts';

export type StickSyncStage = 'idle' | 'planning' | 'confirm' | 'writing' | 'done' | 'failed';

/** One sync at a time: the plan being confirmed, or the write in progress. */
export const stickSync = $state({
  stage: 'idle' as StickSyncStage,
  volumePath: null as string | null,
  plan: null as UsbSyncPlan | null,
  progress: null as StickProgress | null,
  report: null as StickReport | null,
  error: '',
});

export function stickSyncTarget(volumePath: string): SyncTarget {
  return {
    volume: volumePath,
    selection: {
      kind: 'playlists',
      playlist_ids: stickPlaylistIds(volumePath),
      track_ids: stickAllTrackIds(volumePath),
    },
  };
}

/** Ask only when it matters: files would go, it won't fit, or tracks are not analyzed. */
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

/** Called after every finished write, e.g. to re-read the stick. */
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

/** Plans the sync for a stick; writes straight away unless the plan needs the DJ's answer. */
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
    }
  } catch (err) {
    console.info('sync_usb_many failed:', err);
    stickSync.stage = 'failed';
    stickSync.error = syncFailureText(err);
  }
  await loadCapacity(volumePath);
  await onFinished?.(volumePath);
}

const MB = 1_000_000;

export function formatBytes(bytes: number): string {
  if (bytes >= 1000 * MB) return `${(bytes / (1000 * MB)).toFixed(1)} GB`;
  if (bytes >= MB) return `${(bytes / MB).toFixed(1)} MB`;
  return `${Math.max(0, Math.round(bytes / 1000))} KB`;
}

/** Short line for the stick board while or after writing. */
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
