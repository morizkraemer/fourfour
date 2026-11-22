export const PROGRESSIVE_DETAIL_KEY = 'ff.progressiveDetail';

export function loadProgressiveDetail(): boolean {
  try {
    return localStorage.getItem(PROGRESSIVE_DETAIL_KEY) === '1';
  } catch {
    return false;
  }
}

export function saveProgressiveDetail(on: boolean) {
  try {
    if (on) localStorage.setItem(PROGRESSIVE_DETAIL_KEY, '1');
    else localStorage.removeItem(PROGRESSIVE_DETAIL_KEY);
  } catch {
    return;
  }
}
