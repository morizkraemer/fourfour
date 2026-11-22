export type StickStatusKind = 'idle' | 'syncing' | 'pending' | 'wont-fit' | 'error';
export type StickTone = 'none' | 'busy' | 'waiting' | 'problem';

export interface StickSyncSnapshot {
  stage: 'idle' | 'planning' | 'confirm' | 'writing' | 'done' | 'failed';
  volumePath: string | null;
  progress: { done: number; total: number } | null;
}

export interface StickPending {
  changes: number;
  fits: boolean | null;
  edited?: boolean;
}

export interface StickStatus {
  kind: StickStatusKind;
  tone: StickTone;
  label: string;
}

const TONE: Record<StickStatusKind, StickTone> = {
  idle: 'none',
  syncing: 'busy',
  pending: 'waiting',
  'wont-fit': 'problem',
  error: 'problem',
};

function status(kind: StickStatusKind, label: string): StickStatus {
  return { kind, tone: TONE[kind], label };
}

export function changesLabel(changes: number): string {
  return `${changes} ${changes === 1 ? 'change' : 'changes'} waiting`;
}

export function planChanges(plan: { add: unknown[]; remove: unknown[]; update_count: number }): number {
  return plan.add.length + plan.remove.length + plan.update_count;
}

export function stickStatus(volumePath: string, sync: StickSyncSnapshot, pending: StickPending | null | undefined): StickStatus {
  if (sync.volumePath === volumePath) {
    if (sync.stage === 'planning') return status('syncing', 'Checking');
    if (sync.stage === 'writing') {
      const { progress } = sync;
      return status('syncing', progress && progress.total > 0 ? `Syncing ${progress.done}/${progress.total}` : 'Syncing');
    }
    if (sync.stage === 'failed') return status('error', 'Sync failed');
  }
  if (pending?.fits === false) return status('wont-fit', "Won't fit");
  if (pending && pending.changes > 0) return status('pending', changesLabel(pending.changes));
  if (pending?.edited) return status('pending', 'Changes waiting');
  return status('idle', 'In sync');
}

const URGENCY: StickStatusKind[] = ['error', 'wont-fit', 'syncing', 'pending'];

export interface TopbarStick {
  volume: string;
  name: string;
  status: StickStatus;
  others: number;
}

export function stickLabel(volume: string): string {
  return volume.replace(/\\/g, '/').split('/').pop() || volume;
}

export function topbarStick(sticks: { volume: string; status: StickStatus }[]): TopbarStick | null {
  const attention = sticks.filter((s) => s.status.kind !== 'idle');
  if (attention.length === 0) return null;
  const first = [...attention].sort((a, b) => URGENCY.indexOf(a.status.kind) - URGENCY.indexOf(b.status.kind))[0];
  return { volume: first.volume, name: stickLabel(first.volume), status: first.status, others: attention.length - 1 };
}
