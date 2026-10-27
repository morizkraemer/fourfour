export type ResizablePanel = 'sidebar' | 'inspector';

export const PANEL_WIDTH: Record<ResizablePanel, { min: number; max: number; initial: number; key: string }> = {
  sidebar: { min: 200, max: 420, initial: 240, key: 'fourfour.sidebar.width' },
  inspector: { min: 300, max: 520, initial: 340, key: 'fourfour.inspector.width' },
};

export function clampPanelWidth(panel: ResizablePanel, px: number): number {
  const { min, max, initial } = PANEL_WIDTH[panel];
  if (!Number.isFinite(px)) return initial;
  return Math.round(Math.max(min, Math.min(max, px)));
}

export function parseStoredPanelWidth(panel: ResizablePanel, raw: string | null): number {
  if (raw == null) return PANEL_WIDTH[panel].initial;
  return clampPanelWidth(panel, parseFloat(raw));
}

export function loadPanelWidth(panel: ResizablePanel): number {
  try {
    return parseStoredPanelWidth(panel, localStorage.getItem(PANEL_WIDTH[panel].key));
  } catch {
    return PANEL_WIDTH[panel].initial;
  }
}

export function savePanelWidth(panel: ResizablePanel, px: number) {
  try {
    localStorage.setItem(PANEL_WIDTH[panel].key, String(clampPanelWidth(panel, px)));
  } catch {
    return;
  }
}
