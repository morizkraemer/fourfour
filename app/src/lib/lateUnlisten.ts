export function unlistenWhenReady(
  subscription: Promise<() => void>,
  onError: (err: unknown) => void,
): () => void {
  let unlisten: (() => void) | null = null;
  let cancelled = false;
  subscription
    .then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    })
    .catch(onError);
  return () => {
    cancelled = true;
    unlisten?.();
    unlisten = null;
  };
}
