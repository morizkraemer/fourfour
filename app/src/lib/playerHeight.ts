export const PLAYER_HEIGHT = {
  min: 240,
  maxViewportShare: 0.7,
  defaultViewportShare: 0.33,
  defaultMin: 280,
  key: 'fourfour.player.height',
};

export function maxPlayerHeight(viewportHeight: number): number {
  return Math.max(PLAYER_HEIGHT.min, Math.floor(viewportHeight * PLAYER_HEIGHT.maxViewportShare));
}

export function defaultPlayerHeight(viewportHeight: number): number {
  const share = Math.round(viewportHeight * PLAYER_HEIGHT.defaultViewportShare);
  return Math.min(maxPlayerHeight(viewportHeight), Math.max(PLAYER_HEIGHT.defaultMin, share));
}

export function clampPlayerHeight(px: number, viewportHeight: number): number {
  if (!Number.isFinite(px)) return defaultPlayerHeight(viewportHeight);
  return Math.round(Math.max(PLAYER_HEIGHT.min, Math.min(maxPlayerHeight(viewportHeight), px)));
}

export function parseStoredPlayerHeight(raw: string | null): number | null {
  if (raw == null) return null;
  const px = parseFloat(raw);
  return Number.isFinite(px) && px >= PLAYER_HEIGHT.min ? Math.round(px) : null;
}

export function loadPlayerHeight(): number | null {
  try {
    return parseStoredPlayerHeight(localStorage.getItem(PLAYER_HEIGHT.key));
  } catch {
    return null;
  }
}

export function savePlayerHeight(px: number | null) {
  try {
    if (px == null) localStorage.removeItem(PLAYER_HEIGHT.key);
    else localStorage.setItem(PLAYER_HEIGHT.key, String(Math.round(px)));
  } catch {
    return;
  }
}
