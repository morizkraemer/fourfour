import { writeFileSync } from 'node:fs';
import { expect, test, type Browser, type Page } from '@playwright/test';
import type { LoadedState, TrackInfo } from '../src/services/tauri.svelte.ts';
import { fixtureAnalysis, fixtureState } from './fixture.ts';
import { installTauriMock, type SmokeWindow } from './tauri-mock.ts';

const ENABLED = !!process.env.FF_PERF_Q50;
const TRACKS = Number(process.env.FF_PERF_TRACKS ?? 0);
const RUNS = Number(process.env.FF_PERF_RUNS ?? 3);
const IMPORT_FILES = Number(process.env.FF_PERF_IMPORT ?? 1000);
const REPORT = process.env.FF_PERF_Q50_OUT;

const results: Record<string, number> = {};
const record = (name: string, value: number) => (results[name] = Math.round(value * 10) / 10);
const median = (values: number[]) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];

function rowBands(i: number) {
  const bytes = new Uint8Array(128 * 3);
  for (let c = 0; c < 128; c++) {
    bytes[c] = 40 + ((c * 7 + i) % 80);
    bytes[128 + c] = 20 + ((c * 3 + i) % 60);
    bytes[256 + c] = 10 + ((c * 5 + i) % 40);
  }
  return Buffer.from(bytes).toString('base64');
}

function state(): LoadedState {
  const base = fixtureState();
  return { ...base, tracks: base.tracks.map((t, i) => ({ ...t, waveform_bands: rowBands(i) })) };
}

function importedTrack(n: number): TrackInfo {
  return {
    id: 100_000 + n,
    source_path: `/Downloads/Perf Import/track${String(n).padStart(4, '0')}.mp3`,
    title: `Imported ${n}`,
    artist: `Import Artist ${n % 50}`,
    album: `Import Album ${n % 120}`,
    genre: 'House',
    tempo: 0,
    key: '',
    duration_secs: 240,
    bitrate: 320,
    file_size: 9_600_000,
    has_artwork: false,
    has_cues: false,
    has_analysis: false,
    analysis_pending: false,
  };
}

interface Q50 {
  firstRow: number;
  longTasks: { start: number; duration: number }[];
  detailResolved: Record<number, number>;
  mockMs: Record<string, number[]>;
}

function installProbes() {
  const w = window as unknown as { __q50: Q50; __TAURI_INTERNALS__: { invoke: (cmd: string, args: Record<string, unknown>) => Promise<unknown> } };
  w.__q50 = { firstRow: 0, longTasks: [], detailResolved: {}, mockMs: {} };
  new PerformanceObserver((list) => {
    for (const e of list.getEntries()) w.__q50.longTasks.push({ start: e.startTime, duration: e.duration });
  }).observe({ type: 'longtask', buffered: true });
  const watchFirstRow = () => {
    if (document.querySelector('[data-track-row]')) w.__q50.firstRow = performance.now();
    else requestAnimationFrame(watchFirstRow);
  };
  requestAnimationFrame(watchFirstRow);
  const invoke = w.__TAURI_INTERNALS__.invoke;
  w.__TAURI_INTERNALS__.invoke = async (cmd, args = {}) => {
    const started = performance.now();
    const answer = await invoke(cmd, args);
    (w.__q50.mockMs[cmd] ??= []).push(performance.now() - started);
    if (cmd === 'get_waveform_detail') w.__q50.detailResolved[args.trackId as number] = performance.now();
    return answer;
  };
}

async function openApp(browser: Browser, payload: LoadedState) {
  const context = await browser.newContext({ viewport: { width: 1600, height: 1000 } });
  const page = await context.newPage();
  await page.exposeFunction('__smokeAnalysis', (trackId: number) => fixtureAnalysis(trackId));
  await page.addInitScript(installTauriMock, payload);
  await page.addInitScript(installProbes);
  return { context, page };
}

async function bootMetrics(page: Page) {
  await page.locator('[data-track-row]').first().waitFor({ timeout: 120_000 });
  await page.waitForTimeout(2500);
  return page.evaluate(() => {
    const q = (window as unknown as { __q50: Q50 }).__q50;
    const fcp = performance.getEntriesByName('first-contentful-paint')[0]?.startTime ?? 0;
    const tasks = q.longTasks;
    const lastEnd = tasks.reduce((end, t) => Math.max(end, t.start + t.duration), 0);
    return {
      fcp,
      firstRow: q.firstRow,
      interactive: Math.max(q.firstRow, lastEnd),
      tbt: tasks.reduce((sum, t) => sum + Math.max(0, t.duration - 50), 0),
      longestTask: tasks.reduce((m, t) => Math.max(m, t.duration), 0),
      heapMb: ((performance as unknown as { memory?: { usedJSHeapSize: number } }).memory?.usedJSHeapSize ?? 0) / 1048576,
    };
  });
}

type Boot = Awaited<ReturnType<typeof bootMetrics>>;

function recordBoot(prefix: string, runs: Boot[]) {
  record(`${prefix} fcp ms`, median(runs.map((r) => r.fcp)));
  record(`${prefix} first row ms`, median(runs.map((r) => r.firstRow)));
  record(`${prefix} interactive ms`, median(runs.map((r) => r.interactive)));
  record(`${prefix} total blocking ms`, median(runs.map((r) => r.tbt)));
  record(`${prefix} longest task ms`, median(runs.map((r) => r.longestTask)));
  record(`${prefix} js heap mb`, median(runs.map((r) => r.heapMb)));
}

function loadTrack(page: Page, rowIndex: number) {
  return page.evaluate(
    (rowIndex) =>
      new Promise<{ ready: number; painted: number; detail: number; longTask: number }>((resolve, reject) => {
        const q = (window as unknown as { __q50: Q50 }).__q50;
        const row = document.querySelectorAll('[data-track-row]')[rowIndex] as HTMLElement;
        const id = Number(row.dataset.trackId ?? NaN);
        performance.clearMarks('ff:player-ready');
        const tasksBefore = q.longTasks.length;
        const t0 = performance.now();
        row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true }));
        let ready = 0;
        let painted = 0;
        const poll = () => {
          const now = performance.now();
          if (now - t0 > 15_000) return reject(new Error(`track load timed out (ready ${ready}, painted ${painted})`));
          const mark = performance.getEntriesByName('ff:player-ready').at(-1);
          if (mark && !ready) {
            ready = mark.startTime - t0;
            requestAnimationFrame(() => requestAnimationFrame(() => (painted = performance.now() - t0)));
          }
          const detailAt = Number.isNaN(id) ? Math.max(0, ...Object.values(q.detailResolved).filter((v) => v > t0)) : q.detailResolved[id] ?? 0;
          if (painted && detailAt > t0) {
            const longTask = q.longTasks.slice(tasksBefore).reduce((s, t) => s + t.duration, 0);
            return resolve({ ready, painted, detail: detailAt - t0, longTask });
          }
          setTimeout(poll, 0);
        };
        poll();
      }),
    rowIndex,
  );
}

function expandToDetail(page: Page) {
  return page.evaluate(
    () =>
      new Promise<number>((resolve, reject) => {
        const t0 = performance.now();
        (document.querySelector('.ff-player-compact__expand-btn') as HTMLElement).click();
        const poll = () => {
          if (performance.now() - t0 > 15_000) return reject(new Error('expanded detail timed out'));
          const zoom = document.querySelector('.ff-xp .waveform-display__zoom') as HTMLCanvasElement | null;
          if (zoom?.dataset.bands === 'detail') return requestAnimationFrame(() => requestAnimationFrame(() => resolve(performance.now() - t0)));
          requestAnimationFrame(poll);
        };
        poll();
      }),
  );
}

function frameRecorder(page: Page) {
  return page.evaluate(() => {
    const w = window as unknown as { __frames: number[]; __framesStop: boolean };
    w.__frames = [];
    w.__framesStop = false;
    let last = performance.now();
    const tick = () => {
      const t = performance.now();
      w.__frames.push(t - last);
      last = t;
      if (!w.__framesStop) requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  });
}

const stopFrames = (page: Page) =>
  page.evaluate(() => {
    const w = window as unknown as { __frames: number[]; __framesStop: boolean };
    w.__framesStop = true;
    return w.__frames;
  });

test.describe('q50 perf paths', () => {
  test.skip(!ENABLED || TRACKS === 0, 'set FF_PERF_Q50=1 FF_PERF_TRACKS=10000 (scripts/perf.sh does)');
  test.setTimeout(600_000);

  test('startup, track load, import', async ({ browser }) => {
    const payload = state();
    record('tracks', TRACKS);

    const cold: Boot[] = [];
    const warm: Boot[] = [];
    for (let run = 0; run < RUNS; run++) {
      const { context, page } = await openApp(browser, payload);
      await page.goto('/');
      cold.push(await bootMetrics(page));
      await page.reload();
      warm.push(await bootMetrics(page));
      await context.close();
    }
    recordBoot('startup cold', cold);
    recordBoot('startup warm', warm);

    const { context, page } = await openApp(browser, payload);
    await page.goto('/');
    await bootMetrics(page);
    await page.evaluate(() => ((window as unknown as SmokeWindow).__smokeDetail = 'serve'));

    const loads = [];
    const expands = [];
    for (const index of [3, 5, 7, 9, 11]) {
      loads.push(await loadTrack(page, index));
      expands.push(await expandToDetail(page));
      await page.locator('.ff-xp__collapse').click();
      await expect(page.locator('.ff-xp')).toHaveCount(0);
      await page.waitForTimeout(300);
    }
    record('track load to overview data ms', median(loads.map((l) => l.ready)));
    record('track load to overview painted ms', median(loads.map((l) => l.painted)));
    record('track load to detail series ms', median(loads.map((l) => l.detail)));
    record('track load long-task ms', median(loads.map((l) => l.longTask)));
    record('expand to detail drawn ms', median(expands));
    const mockMs = await page.evaluate(() => (window as unknown as { __q50: Q50 }).__q50.mockMs);
    record('mock get_analysis_data ms (not app time)', median(mockMs.get_analysis_data ?? [0]));
    record('mock get_waveform_detail ms (not app time)', median(mockMs.get_waveform_detail ?? [0]));

    await page.evaluate(
      (patch) => Object.assign(window, patch),
      { __smokePickPath: '/Downloads/Perf Import', __smokeImport: { tracks: Array.from({ length: IMPORT_FILES }, (_, n) => importedTrack(n)), already: 0 } },
    );
    await page.waitForTimeout(450);
    await page.locator('[data-library-status]').click();
    const tasksBefore = await page.evaluate(() => (window as unknown as { __q50: Q50 }).__q50.longTasks.length);
    await frameRecorder(page);
    const t0 = await page.evaluate(() => performance.now());
    await page.locator('[data-library-activity-menu] .ff-menu__item').filter({ hasText: 'Import folder…' }).click();
    const total = (TRACKS + IMPORT_FILES).toLocaleString('en-US');
    await expect(page.locator('.ff-gheader__count')).toHaveText(total, { timeout: 60_000 });
    const t1 = await page.evaluate(() => performance.now());
    await page.waitForTimeout(500);
    const frames = await stopFrames(page);
    const tasks = await page.evaluate((n) => (window as unknown as { __q50: Q50 }).__q50.longTasks.slice(n), tasksBefore);
    record('import ui click to rows ms', t1 - t0);
    record('import ui long-task ms', tasks.reduce((s, t) => s + t.duration, 0));
    record('import ui longest task ms', tasks.reduce((m, t) => Math.max(m, t.duration), 0));
    record('import ui worst frame ms', Math.max(...frames));
    await context.close();

    console.log(JSON.stringify(results, null, 2));
    if (REPORT) writeFileSync(REPORT, JSON.stringify(results, null, 2));
  });
});
