import { readFileSync, writeFileSync } from 'node:fs';
import { test, type Browser, type BrowserContext, type Page } from '@playwright/test';
import { installTauriMock } from './tauri-mock.ts';

const PAYLOADS = (process.env.FF_PERF_STARTUP ?? '')
  .split(',')
  .filter(Boolean)
  .map((entry) => {
    const [tracks, path] = entry.split('=');
    return { tracks: Number(tracks), path };
  });
const RUNS = Number(process.env.FF_PERF_RUNS ?? 3);
const REPORT = process.env.FF_PERF_STARTUP_OUT;
const BLOCK_FONTS = !!process.env.FF_PERF_BLOCK_FONTS;

interface Boot {
  firstInvoke: number;
  loadRequested: number;
  transferMs: number;
  parseMs: number;
  loadAnswered: number;
  firstRow: number;
  firstRealRow: number;
  interactive: number;
  longestTask: number;
  tbt: number;
  bundleMs: number;
  wireBytes: number;
}

interface Probe {
  firstInvoke: number;
  loadRequested: number;
  transferMs: number;
  parseMs: number;
  loadAnswered: number;
  firstRow: number;
  firstRealRow: number;
  wireBytes: number;
  longTasks: { start: number; duration: number }[];
}

function installProbes() {
  const w = window as unknown as {
    __boot: Probe;
    __TAURI_INTERNALS__: { invoke: (cmd: string, args: Record<string, unknown>) => Promise<unknown> };
  };
  w.__boot = { firstInvoke: 0, loadRequested: 0, transferMs: 0, parseMs: 0, loadAnswered: 0, firstRow: 0, firstRealRow: 0, wireBytes: 0, longTasks: [] };
  new PerformanceObserver((list) => {
    for (const e of list.getEntries()) w.__boot.longTasks.push({ start: e.startTime, duration: e.duration });
  }).observe({ type: 'longtask', buffered: true });
  const watchRows = () => {
    if (!w.__boot.firstRow && document.querySelector('[data-track-row], [data-boot-row]')) w.__boot.firstRow = performance.now();
    if (document.querySelector('[data-track-row]')) w.__boot.firstRealRow = performance.now();
    else requestAnimationFrame(watchRows);
  };
  requestAnimationFrame(watchRows);
  const invoke = w.__TAURI_INTERNALS__.invoke;
  w.__TAURI_INTERNALS__.invoke = async (cmd, args = {}) => {
    if (!w.__boot.firstInvoke) w.__boot.firstInvoke = performance.now();
    if (cmd !== 'load_state') return invoke(cmd, args);
    w.__boot.loadRequested = performance.now();
    const response = await fetch('/__perf/load_state');
    const text = await response.text();
    const fetched = performance.now();
    const state = JSON.parse(text);
    const parsed = performance.now();
    w.__boot.transferMs = fetched - w.__boot.loadRequested;
    w.__boot.parseMs = parsed - fetched;
    w.__boot.wireBytes = text.length;
    await new Promise((resolve) => setTimeout(resolve, 0));
    w.__boot.loadAnswered = performance.now();
    return state;
  };
}

async function newContext(browser: Browser, body: Buffer, storageState?: Awaited<ReturnType<BrowserContext['storageState']>>) {
  const context = await browser.newContext({ viewport: { width: 1600, height: 1000 }, storageState });
  await context.route('**/__perf/load_state', (route) => route.fulfill({ status: 200, contentType: 'application/json', body }));
  if (BLOCK_FONTS) await context.route(/fonts\.(googleapis|gstatic)\.com/, (route) => route.abort());
  await context.addInitScript(installTauriMock, { tracks: [], playlists: [] });
  await context.addInitScript(installProbes);
  return context;
}

async function bootMetrics(page: Page): Promise<Boot> {
  await page.locator('[data-track-row]').first().waitFor({ timeout: 120_000 });
  await page.waitForTimeout(2500);
  return page.evaluate(() => {
    const b = (window as unknown as { __boot: Probe }).__boot;
    const tasks = b.longTasks;
    const lastEnd = tasks.reduce((end, t) => Math.max(end, t.start + t.duration), 0);
    const scripts = performance.getEntriesByType('resource').filter((e) => e.name.endsWith('.js')) as PerformanceResourceTiming[];
    return {
      firstInvoke: b.firstInvoke,
      loadRequested: b.loadRequested,
      transferMs: b.transferMs,
      parseMs: b.parseMs,
      loadAnswered: b.loadAnswered,
      firstRow: b.firstRow,
      firstRealRow: b.firstRealRow,
      interactive: Math.max(b.firstRealRow, lastEnd),
      longestTask: tasks.reduce((m, t) => Math.max(m, t.duration), 0),
      tbt: tasks.reduce((sum, t) => sum + Math.max(0, t.duration - 50), 0),
      bundleMs: scripts.reduce((end, e) => Math.max(end, e.responseEnd), 0),
      wireBytes: b.wireBytes,
    };
  });
}

const median = (values: number[]) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];

function summarise(runs: Boot[]) {
  const out: Record<string, number> = {};
  for (const key of Object.keys(runs[0]) as (keyof Boot)[]) out[key] = Math.round(median(runs.map((r) => r[key])) * 10) / 10;
  return out;
}

test.describe('startup at scale', () => {
  test.skip(PAYLOADS.length === 0, 'set FF_PERF_STARTUP=10000=/path/state.json,... (bench_startup dump writes them)');
  test.setTimeout(900_000);

  test('boot to first row and interactive', async ({ browser }) => {
    const report: Record<string, Record<string, number>> = {};
    for (const { tracks, path } of PAYLOADS) {
      const body = readFileSync(path);
      const first: Boot[] = [];
      const cold: Boot[] = [];
      const warm: Boot[] = [];
      for (let run = 0; run < RUNS; run++) {
        const seed = await newContext(browser, body);
        const seedPage = await seed.newPage();
        await seedPage.goto('/');
        first.push(await bootMetrics(seedPage));
        await seedPage.close();
        const saved = await seed.storageState();
        await seed.close();

        const context = await newContext(browser, body, saved);
        const page = await context.newPage();
        await page.goto('/');
        cold.push(await bootMetrics(page));
        await page.reload();
        warm.push(await bootMetrics(page));
        await context.close();
      }
      report[`${tracks} first launch`] = summarise(first);
      report[`${tracks} cold`] = summarise(cold);
      report[`${tracks} warm`] = summarise(warm);
    }
    console.log(JSON.stringify(report, null, 2));
    if (REPORT) writeFileSync(REPORT, JSON.stringify(report, null, 2));
  });
});
