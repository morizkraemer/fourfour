import { loadProgressiveDetail, saveProgressiveDetail } from '../lib/progressiveDetail.ts';

export const progressiveDetail = $state({ on: loadProgressiveDetail() });

export function toggleProgressiveDetail(): boolean {
  progressiveDetail.on = !progressiveDetail.on;
  saveProgressiveDetail(progressiveDetail.on);
  return progressiveDetail.on;
}
