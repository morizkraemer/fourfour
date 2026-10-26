import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

function resumeCalls() {
  return invokeMock.mock.calls.filter(([command]) => command === 'resume_analysis');
}

async function freshLibrary() {
  vi.resetModules();
  return import('./library.svelte.ts');
}

beforeEach(() => {
  vi.useFakeTimers();
  invokeMock.mockReset();
  invokeMock.mockResolvedValue(3);
});

afterEach(() => {
  vi.useRealTimers();
});

describe('analysis resume at launch', () => {
  it('waits for the launch delay before asking the backend to resume', async () => {
    const { scheduleLaunchAnalysis, LAUNCH_ANALYSIS_DELAY_MS } = await freshLibrary();
    scheduleLaunchAnalysis();
    await vi.advanceTimersByTimeAsync(LAUNCH_ANALYSIS_DELAY_MS - 1);
    expect(resumeCalls()).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(1);
    expect(resumeCalls()).toHaveLength(1);
  });

  it('schedules once even when called again', async () => {
    const { scheduleLaunchAnalysis, LAUNCH_ANALYSIS_DELAY_MS } = await freshLibrary();
    scheduleLaunchAnalysis();
    scheduleLaunchAnalysis();
    await vi.advanceTimersByTimeAsync(LAUNCH_ANALYSIS_DELAY_MS * 2);
    expect(resumeCalls()).toHaveLength(1);
  });

  it('shows analysis in the status bar when tracks were queued', async () => {
    const { scheduleLaunchAnalysis, library, LAUNCH_ANALYSIS_DELAY_MS } = await freshLibrary();
    library.analyzing = false;
    scheduleLaunchAnalysis();
    await vi.advanceTimersByTimeAsync(LAUNCH_ANALYSIS_DELAY_MS);
    expect(library.analyzing).toBe(true);
    expect(library.statusMessage).toBe('Resuming analysis…');
  });

  it('leaves the status bar alone when nothing was queued', async () => {
    invokeMock.mockResolvedValue(0);
    const { scheduleLaunchAnalysis, library, LAUNCH_ANALYSIS_DELAY_MS } = await freshLibrary();
    library.analyzing = false;
    scheduleLaunchAnalysis();
    await vi.advanceTimersByTimeAsync(LAUNCH_ANALYSIS_DELAY_MS);
    expect(library.analyzing).toBe(false);
  });
});
