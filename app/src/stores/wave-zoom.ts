interface WaveZoom {
  zoomIn(): void;
  zoomOut(): void;
  resetZoom(): void;
}

let active: WaveZoom | null = null;

export function registerWaveZoom(target: WaveZoom): () => void {
  active = target;
  return () => {
    if (active === target) active = null;
  };
}

export const zoomWaveIn = () => active?.zoomIn();
export const zoomWaveOut = () => active?.zoomOut();
export const zoomWaveToFit = () => active?.resetZoom();
