export const UI_BUSY_SIGNAL_EVERY_MS = 200;

export function watchUiBusy(
  target: Pick<Window, 'addEventListener' | 'removeEventListener'>,
  isAnalysing: () => boolean,
  signal: () => void,
  now: () => number = () => performance.now(),
): () => void {
  let lastSignalAt = -Infinity;
  const markBusy = () => {
    if (!isAnalysing()) return;
    const t = now();
    if (t - lastSignalAt < UI_BUSY_SIGNAL_EVERY_MS) return;
    lastSignalAt = t;
    signal();
  };
  const onPointerMove = (event: Event) => {
    if ((event as PointerEvent).buttons !== 0) markBusy();
  };
  const options = { capture: true, passive: true } as const;
  target.addEventListener('scroll', markBusy, options);
  target.addEventListener('wheel', markBusy, options);
  target.addEventListener('pointermove', onPointerMove, options);
  return () => {
    target.removeEventListener('scroll', markBusy, options);
    target.removeEventListener('wheel', markBusy, options);
    target.removeEventListener('pointermove', onPointerMove, options);
  };
}
