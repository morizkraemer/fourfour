import { readFileSync, writeFileSync } from 'node:fs';
import { createServer, type Server } from 'node:http';
import type { AddressInfo } from 'node:net';
import { test, type Browser, type BrowserContext, type Page } from '@playwright/test';
import { installTauriMock } from './tauri-mock.ts';

const PAYLOADS = (process.env.FF_PERF_STARTUP ?? '')
  .split(',')
  .filter(Boolean)
  .map((entry) => {
    const [tracks, path, previews, backendMs] = entry.split('=');
    return { tracks: Number(tracks), path, previews: previews || undefined, backendMs: Number(backendMs ?? 0) };
  });
const RUNS = Number(process.env.FF_PERF_RUNS ?? 3);
const REPORT = process.env.FF_PERF_STARTUP_OUT;
const BLOCK_FONTS = !!process.env.FF_PERF_BLOCK_FONTS;
const NO_SNAPSHOT = !!process.env.FF_PERF_NO_SNAPSHOT;
const PROFILE_OUT = process.env.FF_PERF_STARTUP_PROFILE;
const PROFILE_FIRST = !!process.env.FF_PERF_STARTUP_PROFILE_FIRST;
const SORT_KEY = process.env.FF_PERF_SORT;
const TRACE_OUT = process.env.FF_PERF_STARTUP_TRACE;
const SNAPSHOT_OUT = process.env.FF_PERF_STARTUP_SNAPSHOT;

interface Boot {
  firstInvoke: number;
  loadRequested: number;
  transferMs: number;
  parseMs: number;
  loadAnswered: number;
  decodeMs: number;
  mapMs: number;
  firstRow: number;
  firstRealRow: number;
  interactive: number;
  longestTask: number;
  tbt: number;
  bundleMs: number;
  wireBytes: number;
  frameInteractive: number;
  longestFrame: number;
  heapMB: number;
  cpuMs: number;
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
  frameInteractive: number;
  longestFrame: number;
  longTasks: { start: number; duration: number }[];
}

function installProbes({ payloadUrl, backendMs }: { payloadUrl: string; backendMs: number }) {
  const w = window as unknown as {
    __boot: Probe;
    __TAURI_INTERNALS__: { invoke: (cmd: string, args: Record<string, unknown>) => Promise<unknown> };
  };
  w.__boot = { firstInvoke: 0, loadRequested: 0, transferMs: 0, parseMs: 0, loadAnswered: 0, firstRow: 0, firstRealRow: 0, wireBytes: 0, frameInteractive: 0, longestFrame: 0, longTasks: [] };
  new PerformanceObserver((list) => {
    for (const e of list.getEntries()) w.__boot.longTasks.push({ start: e.startTime, duration: e.duration });
  }).observe({ type: 'longtask', buffered: true });
  const watchFrames = (last: number, streakStart: number, streak: number) => {
    requestAnimationFrame((now) => {
      const gap = now - last;
      w.__boot.longestFrame = Math.max(w.__boot.longestFrame, gap);
      if (gap > 50) return watchFrames(now, now, 0);
      if (streak + 1 >= 10) {
        w.__boot.frameInteractive = streakStart;
        return;
      }
      watchFrames(now, streakStart, streak + 1);
    });
  };
  const watchRows = () => {
    if (!w.__boot.firstRow && document.querySelector('[data-track-row], [data-boot-row]')) w.__boot.firstRow = performance.now();
    if (document.querySelector('[data-track-row]')) {
      w.__boot.firstRealRow = performance.now();
      requestAnimationFrame((now) => watchFrames(now, now, 0));
    } else requestAnimationFrame(watchRows);
  };
  requestAnimationFrame(watchRows);
  const invoke = w.__TAURI_INTERNALS__.invoke;
  w.__TAURI_INTERNALS__.invoke = async (cmd, args = {}) => {
    if (!w.__boot.firstInvoke) w.__boot.firstInvoke = performance.now();
    if (cmd === 'get_row_previews') {
      const ids = (args.trackIds as number[]).join(',');
      return (await fetch(payloadUrl.replace(/\/state\.(json|bin)$/, `/previews?ids=${ids}`))).json();
    }
    if (cmd !== 'load_state') return invoke(cmd, args);
    w.__boot.loadRequested = performance.now();
    if (backendMs > 0) await new Promise((resolve) => setTimeout(resolve, backendMs));
    const fetchStart = performance.now();
    const response = await fetch(payloadUrl);
    if (payloadUrl.endsWith('.bin')) {
      const bytes = await response.arrayBuffer();
      w.__boot.transferMs = performance.now() - fetchStart;
      w.__boot.wireBytes = bytes.byteLength;
      await new Promise((resolve) => setTimeout(resolve, 0));
      w.__boot.loadAnswered = performance.now();
      return bytes;
    }
    const text = await response.text();
    const fetched = performance.now();
    const state = JSON.parse(text);
    const parsed = performance.now();
    w.__boot.transferMs = fetched - fetchStart;
    w.__boot.parseMs = parsed - fetched;
    w.__boot.wireBytes = text.length;
    await new Promise((resolve) => setTimeout(resolve, 0));
    w.__boot.loadAnswered = performance.now();
    return state;
  };
}

type Previews = Map<number, { id: number; waveform_bands?: string; waveform_preview?: string }>;

function previewsOf(path: string | undefined): Previews {
  const previews: Previews = new Map();
  if (!path) return previews;
  for (const t of JSON.parse(readFileSync(path, 'utf8')).tracks) {
    if (t.waveform_bands || t.waveform_preview) previews.set(t.id, { id: t.id, waveform_bands: t.waveform_bands, waveform_preview: t.waveform_preview });
  }
  return previews;
}

function servePayload(body: Buffer, previews: Previews, binary: boolean): Promise<{ server: Server; url: string }> {
  const server = createServer((req, res) => {
    const headers = { 'content-type': 'application/json', 'access-control-allow-origin': '*', 'cache-control': 'no-store' };
    if (req.url?.startsWith('/previews')) {
      const ids = (new URL(req.url, 'http://x').searchParams.get('ids') ?? '').split(',').map(Number);
      res.writeHead(200, headers);
      res.end(JSON.stringify(ids.flatMap((id) => previews.get(id) ?? [])));
      return;
    }
    res.writeHead(200, binary ? { ...headers, 'content-type': 'application/octet-stream' } : headers);
    res.end(body);
  });
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve({ server, url: `http://127.0.0.1:${(server.address() as AddressInfo).port}/state.${binary ? 'bin' : 'json'}` })));
}

async function newContext(browser: Browser, payloadUrl: string, backendMs: number, storageState?: Awaited<ReturnType<BrowserContext['storageState']>>) {
  const context = await browser.newContext({ viewport: { width: 1600, height: 1000 }, storageState });
  if (BLOCK_FONTS) await context.route(/fonts\.(googleapis|gstatic)\.com/, (route) => route.abort());
  if (NO_SNAPSHOT) await context.addInitScript(() => localStorage.removeItem('fourfour.bootSnapshot.v1'));
  if (SORT_KEY) {
    await context.addInitScript((sortKey) => {
      localStorage.setItem('fourfour.tableColumns.v3', JSON.stringify({ layouts: { library: { columns: [], sortKey, sortDir: 'asc' } } }));
    }, SORT_KEY);
  }
  await context.addInitScript(installTauriMock, { tracks: [], playlists: [] });
  await context.addInitScript(installProbes, { payloadUrl, backendMs });
  return context;
}

const metricSessions = new WeakMap<Page, Awaited<ReturnType<BrowserContext['newCDPSession']>>>();

async function mainThreadCpuMs(page: Page): Promise<number> {
  if (page.context().browser()?.browserType().name() !== 'chromium') return 0;
  let cdp = metricSessions.get(page);
  if (!cdp) {
    cdp = await page.context().newCDPSession(page);
    await cdp.send('Performance.enable');
    metricSessions.set(page, cdp);
  }
  const { metrics } = (await cdp.send('Performance.getMetrics')) as { metrics: { name: string; value: number }[] };
  return (metrics.find((m) => m.name === 'ThreadTime')?.value ?? 0) * 1000;
}

async function heapAfterGc(page: Page): Promise<number> {
  if (page.context().browser()?.browserType().name() !== 'chromium') return 0;
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('HeapProfiler.collectGarbage');
  const { usedSize } = (await cdp.send('Runtime.getHeapUsage')) as { usedSize: number };
  await cdp.detach();
  return usedSize / 1048576;
}

async function bootMetrics(page: Page, cpuBase: number): Promise<Boot> {
  await page.locator('[data-track-row]').first().waitFor({ timeout: 120_000 });
  await page.waitForTimeout(2500);
  const cpuMs = (await mainThreadCpuMs(page)) - cpuBase;
  const metrics = await page.evaluate(() => {
    const b = (window as unknown as { __boot: Probe }).__boot;
    const tasks = b.longTasks;
    const lastEnd = tasks.reduce((end, t) => Math.max(end, t.start + t.duration), 0);
    const markTime = (name: string) => performance.getEntriesByName(name).at(-1)?.startTime ?? 0;
    const scripts = performance.getEntriesByType('resource').filter((e) => e.name.endsWith('.js')) as PerformanceResourceTiming[];
    return {
      firstInvoke: b.firstInvoke,
      loadRequested: b.loadRequested,
      transferMs: b.transferMs,
      parseMs: b.parseMs,
      loadAnswered: b.loadAnswered,
      decodeMs: markTime('ff:state-decoded') - markTime('ff:state-answer'),
      mapMs: markTime('ff:state-rows') - markTime('ff:state-decoded'),
      firstRow: b.firstRow,
      firstRealRow: b.firstRealRow,
      interactive: Math.max(b.firstRealRow, lastEnd),
      longestTask: tasks.reduce((m, t) => Math.max(m, t.duration), 0),
      tbt: tasks.reduce((sum, t) => sum + Math.max(0, t.duration - 50), 0),
      bundleMs: scripts.reduce((end, e) => Math.max(end, e.responseEnd), 0),
      wireBytes: b.wireBytes,
      frameInteractive: b.frameInteractive,
      longestFrame: b.longestFrame,
      heapMB: 0,
      cpuMs: 0,
    };
  });
  return { ...metrics, cpuMs, heapMB: await heapAfterGc(page) };
}

async function writeFirstScreen(page: Page, prefix: string) {
  await page.waitForTimeout(1500);
  const html = await page.evaluate(() => document.documentElement.outerHTML);
  writeFileSync(`${prefix}.html`, html);
  writeFileSync(`${prefix}.png`, await page.screenshot());
}

interface InspectorSession {
  send(method: string, params?: Record<string, unknown>): Promise<unknown>;
  on(event: string, listener: (payload: unknown) => void): void;
}

function webkitInspectorSession(page: Page): InspectorSession {
  const internal = page as unknown as { _connection: { toImpl(x: unknown): { delegate: { _session: InspectorSession } } } };
  return internal._connection.toImpl(page).delegate._session;
}

type Profiler =
  | { kind: 'cdp'; cdp: Awaited<ReturnType<BrowserContext['newCDPSession']>> }
  | { kind: 'webkit'; session: InspectorSession; timeline: unknown[]; samples: Promise<unknown> };

async function startProfile(page: Page): Promise<Profiler> {
  if (page.context().browser()?.browserType().name() === 'webkit') {
    const session = webkitInspectorSession(page);
    const timeline: unknown[] = [];
    session.on('Timeline.eventRecorded', (event) => timeline.push(event));
    const samples = new Promise((resolve) => session.on('ScriptProfiler.trackingComplete', resolve));
    await session.send('Timeline.enable');
    await session.send('ScriptProfiler.startTracking', { includeSamples: true });
    await session.send('Timeline.start');
    return { kind: 'webkit', session, timeline, samples };
  }
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Profiler.enable');
  await cdp.send('Profiler.setSamplingInterval', { interval: 200 });
  await cdp.send('Profiler.start');
  return { kind: 'cdp', cdp };
}

async function stopProfile(profiler: Profiler, path: string, page: Page) {
  const marks = await page.evaluate(() => {
    const b = (window as unknown as { __boot: Probe }).__boot;
    return { timeOrigin: performance.timeOrigin, now: performance.now(), loadAnswered: b.loadAnswered, firstRealRow: b.firstRealRow, frameInteractive: b.frameInteractive, longTasks: b.longTasks };
  });
  if (profiler.kind === 'webkit') {
    await profiler.session.send('Timeline.stop');
    await profiler.session.send('ScriptProfiler.stopTracking');
    const samples = await profiler.samples;
    writeFileSync(path.replace(/\.cpuprofile$/, '.webkit.json'), JSON.stringify({ marks, samples, timeline: profiler.timeline }));
    return;
  }
  const { profile } = (await profiler.cdp.send('Profiler.stop')) as { profile: object };
  writeFileSync(path, JSON.stringify({ ...profile, marks }));
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
    for (const { tracks, path, previews, backendMs } of PAYLOADS) {
      const { server, url } = await servePayload(readFileSync(path), previewsOf(previews), path.endsWith('.bin'));
      const first: Boot[] = [];
      const cold: Boot[] = [];
      const warm: Boot[] = [];
      for (let run = 0; run < RUNS; run++) {
        const seed = await newContext(browser, url, backendMs);
        const seedPage = await seed.newPage();
        const profilingFirst = PROFILE_OUT && PROFILE_FIRST && run === RUNS - 1 ? await startProfile(seedPage) : null;
        const seedBase = await mainThreadCpuMs(seedPage);
        await seedPage.goto('/');
        first.push(await bootMetrics(seedPage, seedBase));
        if (profilingFirst) await stopProfile(profilingFirst, `${PROFILE_OUT}-${tracks}-first.cpuprofile`, seedPage);
        await seedPage.close();
        const saved = await seed.storageState();
        await seed.close();

        const context = await newContext(browser, url, backendMs, saved);
        const page = await context.newPage();
        const coldBase = await mainThreadCpuMs(page);
        await page.goto('/');
        cold.push(await bootMetrics(page, coldBase));
        const profiling = PROFILE_OUT && !PROFILE_FIRST && run === RUNS - 1 ? await startProfile(page) : null;
        const warmBase = await mainThreadCpuMs(page);
        const tracing = TRACE_OUT && run === RUNS - 1 && browser.browserType().name() === 'chromium';
        if (tracing) await browser.startTracing(page, { categories: ['devtools.timeline', 'v8', 'v8.execute', 'blink.user_timing', 'disabled-by-default-devtools.timeline'] });
        await page.reload();
        warm.push(await bootMetrics(page, warmBase));
        if (tracing) writeFileSync(`${TRACE_OUT}-${tracks}.json`, await browser.stopTracing());
        if (SNAPSHOT_OUT && run === RUNS - 1) await writeFirstScreen(page, `${SNAPSHOT_OUT}-${tracks}`);
        if (profiling) await stopProfile(profiling, `${PROFILE_OUT}-${tracks}.cpuprofile`, page);
        await context.close();
      }
      server.close();
      report[`${tracks} first launch`] = summarise(first);
      report[`${tracks} cold`] = summarise(cold);
      report[`${tracks} warm`] = summarise(warm);
    }
    console.log(JSON.stringify(report, null, 2));
    if (REPORT) writeFileSync(REPORT, JSON.stringify(report, null, 2));
  });
});
