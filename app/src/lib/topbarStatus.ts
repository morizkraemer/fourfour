export interface TopbarSyncState {
  stage: 'idle' | 'planning' | 'confirm' | 'writing' | 'done' | 'failed';
  volumePath: string | null;
  plan: { add: unknown[]; remove: unknown[]; update_count: number } | null;
}

export interface TopbarStatusInput {
  volumes: string[];
  activeVolume: string | null;
  stickTrackCount: (volume: string) => number;
  sync: TopbarSyncState;
}

export interface TopbarStatus {
  volume: string;
  name: string;
  detail: string;
}

export function stickLabel(volume: string): string {
  return volume.replace(/\\/g, '/').split('/').pop() || volume;
}

function plural(count: number, noun: string): string {
  return `${count} ${noun}${count === 1 ? '' : 's'}`;
}

function stickDetail(input: TopbarStatusInput, volume: string): string {
  const { sync } = input;
  if (sync.volumePath === volume) {
    if (sync.stage === 'planning') return 'checking…';
    if (sync.stage === 'writing') return 'syncing';
    if (sync.stage === 'failed') return 'sync failed';
    if (sync.stage === 'done') return 'in sync';
    if (sync.stage === 'confirm' && sync.plan) {
      const changes = sync.plan.add.length + sync.plan.remove.length + sync.plan.update_count;
      return changes === 0 ? 'in sync' : plural(changes, 'change');
    }
  }
  return plural(input.stickTrackCount(volume), 'track');
}

export function topbarStickStatus(input: TopbarStatusInput): TopbarStatus | null {
  const volume =
    input.activeVolume && input.volumes.includes(input.activeVolume) ? input.activeVolume : input.volumes[0];
  if (!volume) return null;
  const extra = input.volumes.length > 1 ? ` +${input.volumes.length - 1}` : '';
  return { volume, name: `${stickLabel(volume)}${extra}`, detail: stickDetail(input, volume) };
}
