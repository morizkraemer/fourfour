const FALLBACK_MS = 4000;

let release: () => void = () => {};
const pending: (() => Promise<unknown>)[] = [];

const afterFirstScreen = new Promise<void>((resolve) => {
  release = resolve;
});

function nextIdle(): Promise<void> {
  return new Promise((resolve) => {
    if (typeof requestIdleCallback === 'function') requestIdleCallback(() => resolve(), { timeout: 300 });
    else setTimeout(resolve, 50);
  });
}

async function drain() {
  await nextIdle();
  for (let i = 0; i < pending.length; i++) {
    await pending[i]().catch((err) => console.error('preload failed:', err));
    await nextIdle();
  }
}

void afterFirstScreen.then(drain);

export function preloadAfterFirstScreen(start: () => Promise<unknown>) {
  pending.push(start);
}

export function firstScreenShown() {
  requestAnimationFrame(() => requestAnimationFrame(release));
}

export function releaseAfterFallback() {
  setTimeout(release, FALLBACK_MS);
}
