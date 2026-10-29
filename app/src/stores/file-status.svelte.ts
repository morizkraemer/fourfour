import type { FileState } from '../lib/fileHealth.ts';

export const fileStatus = $state({
  byId: {} as Record<number, FileState>,
  total: 0,
  missing: 0,
  error: 0,
  relocated: 0,
  checkedAt: null as number | null,
  durationMs: 0,
  running: false,
  onlyProblems: false,
  dialogOpen: false,
  relocating: false,
  blockedTrackId: null as number | null,
});

const BLOCKED_TIP_MS = 5000;
let blockedTimer: ReturnType<typeof setTimeout> | undefined;

export function noteBlockedLoad(trackId: number) {
  fileStatus.blockedTrackId = trackId;
  clearTimeout(blockedTimer);
  blockedTimer = setTimeout(() => (fileStatus.blockedTrackId = null), BLOCKED_TIP_MS);
}

export function dismissBlockedLoad() {
  clearTimeout(blockedTimer);
  fileStatus.blockedTrackId = null;
}

export function fileStateOf(trackId: number): FileState | undefined {
  return fileStatus.byId[trackId];
}
