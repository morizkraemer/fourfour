import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

const hooks = vi.hoisted(() => ({
  events: {} as Record<string, ((event: { payload: unknown }) => void)[]>,
  resume: undefined as undefined | ((queued: number) => void),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (command: string) => {
    if (command === 'load_state') return { tracks: [], playlists: [] };
    if (command === 'get_app_version') return '0.0.0';
    if (command === 'get_library_path') return '/lib';
    if (command === 'get_mounted_volumes') return [];
    if (command === 'resume_analysis') return new Promise<number>((resolve) => (hooks.resume = resolve));
    return undefined;
  }),
}));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (name: string, callback: (event: { payload: unknown }) => void) => {
    (hooks.events[name] ??= []).push(callback);
    return () => {};
  }),
}));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

const COLD_IMPORT_TIMEOUT_MS = 60_000;

beforeAll(async () => {
  await import('./library.svelte.ts');
}, COLD_IMPORT_TIMEOUT_MS);

beforeEach(() => {
  Object.assign(globalThis, { window: { addEventListener() {}, removeEventListener() {} } });
  vi.useFakeTimers();
  hooks.events = {};
  hooks.resume = undefined;
});

afterEach(() => {
  vi.useRealTimers();
});

function emit(name: string, payload: unknown = undefined) {
  hooks.events[name]?.forEach((callback) => callback({ payload }));
}

describe('analysis resume at launch', () => {
  it('does not show analysis as running when the run ended before the backend answered', async () => {
    vi.resetModules();
    const { initLibrary, library, LAUNCH_ANALYSIS_DELAY_MS } = await import('./library.svelte.ts');
    await initLibrary();
    await vi.advanceTimersByTimeAsync(LAUNCH_ANALYSIS_DELAY_MS);
    expect(hooks.resume).toBeDefined();

    emit('analysis-complete');
    hooks.resume!(1);
    await vi.advanceTimersByTimeAsync(0);

    expect(library.analyzing).toBe(false);
  });
});
