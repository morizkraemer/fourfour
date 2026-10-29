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
});

export function fileStateOf(trackId: number): FileState | undefined {
  return fileStatus.byId[trackId];
}
