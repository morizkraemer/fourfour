import { appendFileSync, readFileSync, writeFileSync } from 'node:fs';
import { loadavg } from 'node:os';
import { test, type Browser, type CDPSession, type Page } from '@playwright/test';
import type { LoadedState, TrackInfo } from '../src/services/tauri.svelte.ts';
import { decodeTrackTable, isTrackTable } from '../src/services/track-table.ts';
import { mockAnalysisData } from '../src/mocks/analysis.ts';
import { installTauriMock, type SmokeWindow } from './tauri-mock.ts';

const STATE = process.env.FF_PERF_STATE;
const PARTS = new Set((process.env.FF_PERF_BUDGET ?? '').split(',').filter(Boolean));
const REPORT = process.env.FF_PERF_BUDGET_OUT;
const LABEL = process.env.FF_PERF_LABEL ?? '';
const RUNS = Number(process.env.FF_PERF_RUNS ?? 3);
const WEBKIT = process.env.FF_SMOKE_BROWSER === 'webkit';
const PROFILE = !!process.env.FF_PERF_PROFILE;
const IMPORT_FILES = Number(process.env.FF_PERF_IMPORT ?? 1000);
const WINDOW_SECONDS = Number(process.env.FF_PERF_WINDOW ?? 15);
const SOAK_MINUTES = Number(process.env.FF_PERF_SOAK_MIN ?? 10);

const median = (values: number[]) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)] ?? 0;
const percentile = (values: number[], p: number) => [...values].sort((a, b) => a - b)[Math.min(values.length - 1, Math.floor((values.length * p) / 100))] ?? 0;
const round = (value: number) => Math.round(value * 10) / 10;

function report(kind: string, run: number, data: unknown) {
  const line = JSON.stringify({ label: LABEL, kind, run, browser: WEBKIT ? 'webkit' : 'chromium', profile: PROFILE, load: loadavg().map(round), data });
  if (REPORT) appendFileSync(REPORT, `${line}\n`);
  else console.log(line);
}

function state(): LoadedState {
  const dumped = JSON.parse(readFileSync(STATE!, 'utf8'));
  return { ...dumped, tracks: isTrackTable(dumped.tracks) ? decodeTrackTable(dumped.tracks) : dumped.tracks } as LoadedState;
}

function importedTrack(n: number): TrackInfo {
  return {
    id: 1_000_000 + n,
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

interface Probe {
  t0: number;
  firstRow: number;
  longTasks: { start: number; duration: number }[];
}

function installProbes() {
  const w = window as unknown as Window & { __p: Probe; __awaitFlow: (src: string, before: unknown, timeout?: number) => Promise<number> };
  w.__p = { t0: 0, firstRow: 0, longTasks: [] };
  try {
    new PerformanceObserver((list) => list.getEntries().forEach((e) => w.__p.longTasks.push({ start: e.startTime, duration: e.duration }))).observe({ type: 'longtask', buffered: true });
  } catch {
    w.__p.longTasks = [];
  }
  const watch = () => {
    if (document.querySelector('[data-track-row]')) w.__p.firstRow = performance.now();
    else requestAnimationFrame(watch);
  };
  requestAnimationFrame(watch);
  for (const type of ['pointerdown', 'click', 'dblclick', 'keydown']) document.addEventListener(type, (e) => (w.__p.t0 = e.timeStamp), true);
  const internals = (window as unknown as { __TAURI_INTERNALS__: { invoke: (cmd: string, args: Record<string, unknown>) => Promise<unknown> } }).__TAURI_INTERNALS__;
  const invoke = internals.invoke;
  let detail: ArrayBuffer | null = null;
  (w as unknown as { __prewarmDetail: () => Promise<void> }).__prewarmDetail = async () => {
    detail = (await invoke('get_waveform_detail', { trackId: 1 })) as ArrayBuffer;
  };
  internals.invoke = (cmd, args = {}) => (cmd === 'get_waveform_detail' && detail ? Promise.resolve(detail.slice(0)) : invoke(cmd, args));
  w.__awaitFlow = (src, before, timeout = 15_000) =>
    new Promise((resolve) => {
      const predicate = new Function('__before', `return (${src})`) as (before: unknown) => boolean;
      const t0 = w.__p.t0;
      const started = performance.now();
      const poll = () => {
        if (predicate(before)) return requestAnimationFrame(() => requestAnimationFrame(() => resolve(performance.now() - t0)));
        if (performance.now() - started > timeout) return resolve(-1);
        requestAnimationFrame(poll);
      };
      requestAnimationFrame(poll);
    });
}

async function openApp(browser: Browser, payload: LoadedState) {
  const context = await browser.newContext({ viewport: { width: 1600, height: 1000 } });
  const page = await context.newPage();
  await page.exposeFunction('__smokeAnalysisRemote', (trackId: number) => mockAnalysisData(trackId, 180 + ((trackId * 7) % 300), 12400, '8A'));
  await page.addInitScript(() => {
    const w = window as unknown as { __smokeAnalysisCache: Map<number, unknown>; __smokeAnalysisRemote: (id: number) => Promise<unknown>; __smokeAnalysis: (id: number) => Promise<unknown> };
    w.__smokeAnalysisCache = new Map();
    w.__smokeAnalysis = (id) => {
      if (w.__smokeAnalysisCache.has(id)) return Promise.resolve(w.__smokeAnalysisCache.get(id));
      const local = (window as unknown as { __smokeAnalysisTemplate?: unknown }).__smokeAnalysisTemplate;
      return local ? Promise.resolve(local) : w.__smokeAnalysisRemote(id);
    };
  });
  await page.addInitScript(installTauriMock, payload);
  await page.addInitScript(installProbes);
  const cdp = WEBKIT ? null : await context.newCDPSession(page);
  if (cdp) await cdp.send('Performance.enable');
  return { context, page, cdp };
}

type Counters = Record<'script' | 'layout' | 'style' | 'task', number>;

async function counters(cdp: CDPSession | null): Promise<Counters> {
  if (!cdp) return { script: 0, layout: 0, style: 0, task: 0 };
  const { metrics } = (await cdp.send('Performance.getMetrics')) as { metrics: { name: string; value: number }[] };
  const get = (name: string) => (metrics.find((m) => m.name === name)?.value ?? 0) * 1000;
  return { script: get('ScriptDuration'), layout: get('LayoutDuration'), style: get('RecalcStyleDuration'), task: get('TaskDuration') };
}

const diff = (a: Counters, b: Counters): Counters => ({ script: b.script - a.script, layout: b.layout - a.layout, style: b.style - a.style, task: b.task - a.task });
const restOf = (c: Counters) => round(Math.max(0, c.task - c.script - c.layout - c.style));

interface Profile {
  nodes: { id: number; callFrame: { functionName: string; url: string; lineNumber: number } }[];
  samples: number[];
  timeDeltas: number[];
}

function topFunctions(profile: Profile, count = Number(process.env.FF_PERF_TOP ?? 3)) {
  const byId = new Map(profile.nodes.map((n) => [n.id, n]));
  const self = new Map<string, number>();
  profile.samples.forEach((id, i) => {
    const frame = byId.get(id)!.callFrame;
    if (['(program)', '(idle)', '(root)'].includes(frame.functionName)) return;
    const key = `${frame.functionName || '(anon)'} ${frame.url.split('/').pop() ?? ''}`.trim();
    self.set(key, (self.get(key) ?? 0) + profile.timeDeltas[i] / 1000);
  });
  return [...self.entries()]
    .sort((a, b) => b[1] - a[1])
    .slice(0, count)
    .map(([name, ms]) => `${name} ${round(ms)}`);
}

async function profileStart(cdp: CDPSession | null) {
  if (!cdp || !PROFILE) return;
  await cdp.send('Profiler.enable');
  await cdp.send('Profiler.setSamplingInterval', { interval: 200 });
  await cdp.send('Profiler.start');
}

async function profileStop(cdp: CDPSession | null) {
  if (!cdp || !PROFILE) return [];
  return topFunctions(((await cdp.send('Profiler.stop')) as { profile: Profile }).profile);
}

const longTasksSince = (page: Page, from: number) => page.evaluate((n) => (window as unknown as { __p: Probe }).__p.longTasks.slice(n), from);
const longTaskCount = (page: Page) => page.evaluate(() => (window as unknown as { __p: Probe }).__p.longTasks.length);
const sumMs = (tasks: { duration: number }[]) => round(tasks.reduce((s, t) => s + t.duration, 0));

interface Step {
  name: string;
  ms: number;
  script: number;
  layoutStyle: number;
  rest: number;
  longTaskMs: number;
  top: string[];
}

async function measured(page: Page, cdp: CDPSession | null, name: string, action: () => Promise<void>, predicate: string, before: unknown = null): Promise<Step> {
  await profileStart(cdp);
  const tasks0 = await longTaskCount(page);
  const c0 = await counters(cdp);
  await action();
  const ms = await page.evaluate(([src, b]) => (window as unknown as { __awaitFlow: (s: string, b: unknown) => Promise<number> }).__awaitFlow(src as string, b), [predicate, before]);
  const c = diff(c0, await counters(cdp));
  const top = await profileStop(cdp);
  return { name, ms: round(ms), script: round(c.script), layoutStyle: round(c.layout + c.style), rest: restOf(c), longTaskMs: sumMs(await longTasksSince(page, tasks0)), top };
}

async function boot(page: Page, cdp: CDPSession | null) {
  await page.goto('/');
  await page.locator('[data-track-row]').first().waitFor({ timeout: 120_000 });
  if (process.env.FF_PERF_FRAMES) await page.evaluate(() => { const frame = () => requestAnimationFrame(frame); frame(); });
  const atRows = await counters(cdp);
  const firstRow = await page.evaluate(() => (window as unknown as { __p: Probe }).__p.firstRow);
  return { firstRow, atRows };
}

const SORT_LABEL = (key: string) => `.ff-colh__cell[data-col-key="${key}"] .ff-colh__label`;
const FIRST_ROW_TEXT = `document.querySelector('[data-track-row]')?.textContent !== __before`;
const firstRowText = (page: Page) => page.evaluate(() => document.querySelector('[data-track-row]')?.textContent ?? '');

function biggestPlaylist(payload: LoadedState) {
  return [...payload.playlists].filter((p) => !p.is_folder).sort((a, b) => b.track_ids.length - a.track_ids.length)[0];
}

async function sidebarRow(page: Page, name: string) {
  const row = page.locator('[data-drop-track-playlist-id]').filter({ has: page.locator('.ff-sbrow__label', { hasText: new RegExp(`^${name}$`) }) }).first();
  await row.scrollIntoViewIfNeeded();
  return row;
}

function scrollFling(page: Page, frames: number, mode: 'raf' | 'wheel' | 'steady') {
  return page.evaluate(
    ({ frames, mode }) =>
      new Promise<number[]>((resolve) => {
        const el = document.querySelector('.ff-table__scroll') as HTMLElement;
        el.scrollTop = 0;
        const step = mode === 'steady' ? 90 : (el.scrollHeight - el.clientHeight) / frames;
        const deltas: number[] = [];
        let last = performance.now();
        let n = 0;
        const tick = () => {
          const t = performance.now();
          deltas.push(t - last);
          last = t;
          if (mode !== 'wheel') el.scrollTop += step;
          if (++n < frames) requestAnimationFrame(tick);
          else resolve(deltas);
        };
        requestAnimationFrame(tick);
        if (mode === 'wheel') {
          let sent = 0;
          const wheel = () => {
            el.scrollTop += step;
            if (++sent < frames) setTimeout(wheel, 16);
          };
          wheel();
        }
      }),
    { frames, mode },
  );
}

const jank = (frames: number[]) => ({
  frames: frames.length,
  p50: round(median(frames)),
  p95: round(percentile(frames, 95)),
  max: round(Math.max(...frames)),
  over25: frames.filter((f) => f > 25).length,
  over50: frames.filter((f) => f > 50).length,
  totalMs: round(frames.reduce((s, f) => s + f, 0)),
});

async function heap(page: Page, cdp: CDPSession | null) {
  const pre = await page.evaluate(() => (performance as unknown as { memory?: { usedJSHeapSize: number } }).memory?.usedJSHeapSize ?? 0);
  if (!cdp) return { preGcMb: round(pre / 1048576), mb: 0, nodes: 0 };
  await cdp.send('HeapProfiler.collectGarbage');
  const usage = (await cdp.send('Runtime.getHeapUsage')) as { usedSize: number; totalSize: number };
  const { metrics } = (await cdp.send('Performance.getMetrics')) as { metrics: { name: string; value: number }[] };
  return { preGcMb: round(pre / 1048576), mb: round(usage.usedSize / 1048576), totalMb: round(usage.totalSize / 1048576), nodes: metrics.find((m) => m.name === 'Nodes')?.value ?? 0 };
}

const PREFETCH_EVENTS = 250;

async function answerAnalysisLocally(page: Page) {
  await page.evaluate(async () => {
    const w = window as unknown as { __smokeAnalysisRemote: (id: number) => Promise<unknown>; __smokeAnalysisTemplate?: unknown };
    w.__smokeAnalysisTemplate = await w.__smokeAnalysisRemote(1);
  });
}

async function startAnalysis(page: Page, ids: number[], perSecond: number) {
  await page.evaluate(
    async ({ ids, count }) => {
      const w = window as unknown as { __smokeAnalysisCache: Map<number, unknown>; __smokeAnalysisRemote: (id: number) => Promise<unknown> };
      for (let n = 0; n < count; n++) {
        const id = ids[(n * 7919) % ids.length];
        if (!w.__smokeAnalysisCache.has(id)) w.__smokeAnalysisCache.set(id, await w.__smokeAnalysisRemote(id));
      }
    },
    { ids, count: PREFETCH_EVENTS },
  );
  await page.evaluate(
    ({ ids, perSecond }) => {
      const w = window as unknown as SmokeWindow & { __emitter?: number; __cursor?: number };
      if (w.__emitter) clearInterval(w.__emitter);
      w.__cursor = 0;
      w.__emitter = window.setInterval(() => {
        const n = w.__cursor!++;
        const track_id = ids[(n * 7919) % ids.length];
        w.__smokeEmit('analysis-partial', { track_id });
        if (n % 6 === 5) w.__smokeEmit('analysis-progress', { current: n, total: ids.length, message: `Analyzed track ${track_id}`, track_id });
      }, 1000 / perSecond);
    },
    { ids, perSecond },
  );
}

const stopAnalysis = (page: Page) => page.evaluate(() => clearInterval((window as unknown as { __emitter?: number }).__emitter));

test.describe('r152 budget', () => {
  test.skip(!STATE || PARTS.size === 0, 'set FF_PERF_STATE=/path/state.json FF_PERF_BUDGET=boot,flows,import,analysis,memory [FF_PERF_PROFILE=1]');
  test.setTimeout(3_600_000);

  test('boot: cold open to first rows and to the first answered click', async ({ browser }) => {
    test.skip(!PARTS.has('boot'));
    const payload = state();
    for (let run = 0; run < RUNS; run++) {
      const { context, page, cdp } = await openApp(browser, payload);
      const { firstRow, atRows } = await boot(page, cdp);
      const click = await measured(page, cdp, 'first click (title sort) right after first rows', () => page.locator(SORT_LABEL('title')).click(), FIRST_ROW_TEXT, await firstRowText(page));
      const interactive = await page.evaluate(() => performance.now());
      await page.waitForTimeout(3000);
      const p = await page.evaluate(() => {
        const tasks = (window as unknown as { __p: Probe }).__p.longTasks;
        return {
          fcp: performance.getEntriesByName('first-contentful-paint')[0]?.startTime ?? 0,
          lastTaskEnd: tasks.reduce((end, t) => Math.max(end, t.start + t.duration), 0),
          tbt: tasks.reduce((s, t) => s + Math.max(0, t.duration - 50), 0),
          longest: tasks.reduce((m, t) => Math.max(m, t.duration), 0),
          taskCount: tasks.length,
        };
      });
      const end = await counters(cdp);
      report('boot', run, {
        firstRow: round(firstRow),
        fcp: round(p.fcp),
        interactiveClickAnswered: round(interactive),
        quietAfter: round(p.lastTaskEnd),
        tbt: round(p.tbt),
        longest: round(p.longest),
        taskCount: p.taskCount,
        toFirstRows: { script: round(atRows.script), layoutStyle: round(atRows.layout + atRows.style), rest: restOf(atRows) },
        mainThreadAfterFirstRows: round(end.task - atRows.task),
        click,
        heap: await heap(page, cdp),
      });
      await context.close();
    }
  });

  test('tasks: long tasks after the first rows, no input', async ({ browser }) => {
    test.skip(!PARTS.has('tasks'));
    const payload = state();
    for (let run = 0; run < RUNS; run++) {
      const { context, page, cdp } = await openApp(browser, payload);
      const { firstRow } = await boot(page, cdp);
      await profileStart(cdp);
      await page.waitForTimeout(Number(process.env.FF_PERF_TASKS_MS ?? 6000));
      const top = await profileStop(cdp);
      const tasks = await page.evaluate(() => (window as unknown as { __p: Probe }).__p.longTasks);
      report('tasks', run, { firstRow: round(firstRow), tasks: tasks.map((t) => [round(t.start - firstRow), round(t.duration)]), top });
      await context.close();
    }
  });

  test('early: sort click and row select at a fixed delay after the first rows', async ({ browser }) => {
    test.skip(!PARTS.has('early'));
    const payload = state();
    const delays = (process.env.FF_PERF_CLICK_AT ?? '0,100,300').split(',').map(Number);
    const detailChunkLoaded = (page: Page) => page.evaluate(() => performance.getEntriesByType('resource').some((e) => /Detail/.test(e.name)));
    for (let run = 0; run < RUNS; run++) {
      for (const delay of delays) {
        for (const action of (process.env.FF_PERF_ACTIONS ?? 'sort,select').split(',') as ('sort' | 'select')[]) {
          const { context, page, cdp } = await openApp(browser, payload);
          await boot(page, cdp);
          if (action === 'select') await answerAnalysisLocally(page);
          if (delay > 0) await page.waitForTimeout(delay);
          const chunkBefore = await detailChunkLoaded(page);
          const wanted = await page.evaluate(() => document.querySelectorAll('[data-track-row]')[4].querySelector('.ff-trow__cell--title .ff-trow__text')!.textContent!.trim());
          const step =
            action === 'sort'
              ? await measured(page, cdp, `sort title at +${delay}`, () => page.locator(SORT_LABEL('title')).click(), FIRST_ROW_TEXT, await firstRowText(page))
              : await measured(page, cdp, `select row at +${delay}`, () => page.locator('[data-track-row]').nth(4).click(), `document.querySelector('.ff-inspector input[aria-label="Title"]')?.value === ${JSON.stringify(wanted)}`);
          report('early', run, { action, delay, chunkBefore, step });
          await context.close();
        }
      }
    }
  });

  test('flows: first click on sort, playlist, track, expand, analysis panel, settings, scroll', async ({ browser }) => {
    test.skip(!PARTS.has('flows'));
    const payload = state();
    const playlist = biggestPlaylist(payload);
    for (let run = 0; run < RUNS; run++) {
      const { context, page, cdp } = await openApp(browser, payload);
      await boot(page, cdp);
      await page.evaluate(() => ((window as unknown as SmokeWindow).__smokeDetail = 'serve'));
      await page.waitForTimeout(8000);
      const steps: Step[] = [];
      const gap = () => page.waitForTimeout(900);
      await answerAnalysisLocally(page);

      steps.push(await measured(page, cdp, 'sort header (artist, first click)', () => page.locator(SORT_LABEL('artist')).click(), FIRST_ROW_TEXT, await firstRowText(page)));
      await gap();
      steps.push(await measured(page, cdp, 'sort header (artist, second click)', () => page.locator(SORT_LABEL('artist')).click(), FIRST_ROW_TEXT, await firstRowText(page)));
      await gap();
      const total = (await page.locator('.ff-gheader__count').innerText()).trim();
      const row = await sidebarRow(page, playlist.name);
      steps.push(await measured(page, cdp, `playlist (${playlist.track_ids.length} tracks)`, () => row.click(), `document.querySelector('.ff-gheader__count')?.textContent?.trim() !== __before`, total));
      await gap();
      const wanted = await page.evaluate(() => document.querySelectorAll('[data-track-row]')[4].querySelector('.ff-trow__cell--title .ff-trow__text')!.textContent!.trim());
      steps.push(await measured(page, cdp, 'select row to inspector', () => page.locator('[data-track-row]').nth(4).click(), `document.querySelector('.ff-inspector input[aria-label="Title"]')?.value === ${JSON.stringify(wanted)}`));
      await gap();
      await page.evaluate(() => (window as unknown as { __prewarmDetail: () => Promise<void> }).__prewarmDetail());
      const marks = await page.evaluate(() => performance.getEntriesByName('ff:player-ready').length);
      steps.push(await measured(page, cdp, 'track double click to player ready', () => page.locator('[data-track-row]').nth(6).dblclick(), `performance.getEntriesByName('ff:player-ready').length > __before`, marks));
      await page.waitForTimeout(1500);
      steps.push(await measured(page, cdp, 'expand big player', () => page.locator('.ff-player-compact__expand-btn').click(), `!!document.querySelector('.ff-xp .waveform-display__zoom')`));
      await page.waitForTimeout(1500);
      await page.locator('.ff-xp__collapse').click();
      await page.waitForTimeout(500);
      steps.push(await measured(page, cdp, 'analysis panel (activity menu)', () => page.locator('[data-library-status]').click(), `!!document.querySelector('[data-library-activity-menu]')`));
      await page.keyboard.press('Escape');
      await gap();
      steps.push(await measured(page, cdp, 'settings', () => page.locator('button[aria-label="Settings"]').click(), `!!document.querySelector('.ff-settings-sheet__heading')`));
      await page.keyboard.press('Escape');
      await gap();

      const scrolls: Record<string, unknown> = {};
      for (const mode of ['steady', 'raf', 'wheel'] as const) {
        await page.locator('.ff-sbrow__label', { hasText: /^All Tracks$/ }).click();
        await page.waitForTimeout(800);
        await profileStart(cdp);
        const tasks0 = await longTaskCount(page);
        const c0 = await counters(cdp);
        const frames = await scrollFling(page, mode === 'raf' ? 90 : 120, mode);
        const c = diff(c0, await counters(cdp));
        const tasks = await longTasksSince(page, tasks0);
        scrolls[mode] = { ...jank(frames), script: round(c.script), layoutStyle: round(c.layout + c.style), rest: restOf(c), longTaskMs: sumMs(tasks), longTasks: tasks.length, top: await profileStop(cdp) };
        await page.waitForTimeout(800);
      }
      report('flows', run, { steps, scrolls, heapAfterScroll: await heap(page, cdp) });
      await context.close();
    }
  });

  test('import: 1k folder into the library', async ({ browser }) => {
    test.skip(!PARTS.has('import'));
    const payload = state();
    const count = payload.tracks.length;
    for (let run = 0; run < RUNS; run++) {
      const { context, page, cdp } = await openApp(browser, payload);
      await boot(page, cdp);
      await page.waitForTimeout(6000);
      await page.evaluate((patch) => Object.assign(window, patch), { __smokePickPath: '/Downloads/Perf Import', __smokeImport: { tracks: Array.from({ length: IMPORT_FILES }, (_, n) => importedTrack(n)), already: 0 } });
      await page.locator('[data-library-status]').click();
      await page.waitForTimeout(450);
      const total = (count + IMPORT_FILES).toLocaleString('en-US');
      const tasks0 = await longTaskCount(page);
      await profileStart(cdp);
      const c0 = await counters(cdp);
      await page.evaluate(() => {
        const w = window as unknown as { __frames: number[]; __stop: boolean };
        w.__frames = [];
        w.__stop = false;
        let last = performance.now();
        const tick = () => {
          const t = performance.now();
          w.__frames.push(t - last);
          last = t;
          if (!w.__stop) requestAnimationFrame(tick);
        };
        requestAnimationFrame(tick);
      });
      const { t0, t1 } = await page.evaluate(
        (total) =>
          new Promise<{ t0: number; t1: number }>((resolve, reject) => {
            const item = [...document.querySelectorAll('[data-library-activity-menu] .ff-menu__item')].find((e) => e.textContent?.includes('Import folder')) as HTMLElement;
            const t0 = performance.now();
            item.click();
            const poll = () => {
              if (performance.now() - t0 > 90_000) return reject(new Error('import never showed the rows'));
              if (document.querySelector('.ff-gheader__count')?.textContent?.trim() === total) return resolve({ t0, t1: performance.now() });
              requestAnimationFrame(poll);
            };
            requestAnimationFrame(poll);
          }),
        total,
      );
      await page.waitForTimeout(3000);
      const frames = await page.evaluate(() => {
        const w = window as unknown as { __frames: number[]; __stop: boolean };
        w.__stop = true;
        return w.__frames;
      });
      const c = diff(c0, await counters(cdp));
      const tasks = await longTasksSince(page, tasks0);
      const lastEnd = tasks.reduce((end, t) => Math.max(end, t.start + t.duration), 0);
      report('import', run, {
        toRowsMs: round(t1 - t0),
        doneMs: round(Math.max(t1, lastEnd) - t0),
        script: round(c.script),
        layoutStyle: round(c.layout + c.style),
        rest: restOf(c),
        longTaskMs: sumMs(tasks),
        longest: round(tasks.reduce((m, t) => Math.max(m, t.duration), 0)),
        worstFrame: round(Math.max(...frames)),
        top: await profileStop(cdp),
      });
      await context.close();
    }
  });

  test('analysis: main-thread busy per second, list and player', async ({ browser }) => {
    test.skip(!PARTS.has('analysis') || WEBKIT);
    const payload = state();
    const ids = payload.tracks.map((t) => t.id);
    for (let run = 0; run < RUNS; run++) {
      const { context, page, cdp } = await openApp(browser, payload);
      await boot(page, cdp);
      await page.waitForTimeout(10_000);
      const windows: Record<string, { busyPerSecond: number[]; scriptPerSecond: number[]; top: string[] }> = {};
      const sample = async (name: string) => {
        const busy: number[] = [];
        const script: number[] = [];
        await profileStart(cdp);
        let last = await counters(cdp);
        for (let s = 0; s < WINDOW_SECONDS; s++) {
          await page.waitForTimeout(1000);
          const now = await counters(cdp);
          busy.push(round(now.task - last.task));
          script.push(round(now.script - last.script));
          last = now;
        }
        windows[name] = { busyPerSecond: busy, scriptPerSecond: script, top: await profileStop(cdp) };
      };
      await sample('list, no analysis');
      await startAnalysis(page, ids, 1);
      await page.waitForTimeout(2000);
      await sample('list, analysis 1 event/s');
      await startAnalysis(page, ids, 6);
      await page.waitForTimeout(2000);
      await sample('list, analysis 6 events/s');
      await stopAnalysis(page);
      await page.evaluate(() => ((window as unknown as SmokeWindow).__smokeDetail = 'serve'));
      await page.locator('[data-track-row]').nth(6).dblclick();
      await page.locator('.ff-player-compact__meta-title').waitFor();
      await page.waitForTimeout(1500);
      await page.keyboard.press('Space');
      await page.waitForTimeout(2000);
      await sample('player playing (strip), no analysis');
      await startAnalysis(page, ids, 1);
      await page.waitForTimeout(2000);
      await sample('player playing (strip), analysis 1 event/s');
      await page.locator('.ff-player-compact__expand-btn').click();
      await page.waitForTimeout(2500);
      await sample('big player playing, analysis 1 event/s');
      await stopAnalysis(page);
      await page.waitForTimeout(1000);
      await sample('big player playing, no analysis');
      report('analysis', run, windows);
      await context.close();
    }
  });

  test('analysis-list: main-thread busy and GC per second, list visible, 1 and 6 events/s', async ({ browser }) => {
    test.skip(!PARTS.has('analysis-list') || WEBKIT);
    const payload = state();
    const ids = payload.tracks.map((t) => t.id);
    for (let run = 0; run < RUNS; run++) {
      const { context, page, cdp } = await openApp(browser, payload);
      await boot(page, cdp);
      await page.waitForTimeout(10_000);
      const windows: Record<string, { busyPerSecond: number[]; scriptPerSecond: number[]; layoutStylePerSecond: number[]; gcMsPerSecond: number; eventsPerSecond: number; top: string[] }> = {};
      const sample = async (name: string, eventsPerSecond: number) => {
        const busy: number[] = [];
        const script: number[] = [];
        const layoutStyle: number[] = [];
        await cdp!.send('Profiler.enable');
        await cdp!.send('Profiler.setSamplingInterval', { interval: 1000 });
        await cdp!.send('Profiler.start');
        const traced: unknown[] = [];
        if (process.env.FF_PERF_TRACE && eventsPerSecond > 0) {
          cdp!.on('Tracing.dataCollected', (e: { value: unknown[] }) => traced.push(...e.value));
          await cdp!.send('Tracing.start', { traceConfig: { includedCategories: ['devtools.timeline', 'v8', 'disabled-by-default-devtools.timeline', 'blink', 'cc', 'toplevel'] } });
        }
        let last = await counters(cdp);
        for (let s = 0; s < WINDOW_SECONDS; s++) {
          await page.waitForTimeout(1000);
          const now = await counters(cdp);
          busy.push(round(now.task - last.task));
          script.push(round(now.script - last.script));
          layoutStyle.push(round(now.layout - last.layout + now.style - last.style));
          last = now;
        }
        if (traced.length === 0 && process.env.FF_PERF_TRACE && eventsPerSecond > 0) traced.push(0);
        if (process.env.FF_PERF_TRACE && eventsPerSecond > 0) {
          const done = new Promise((r) => cdp!.once('Tracing.tracingComplete', r));
          await cdp!.send('Tracing.end');
          await done;
          writeFileSync(`${process.env.FF_PERF_TRACE}-${eventsPerSecond}.json`, JSON.stringify(traced));
        }
        const profile = ((await cdp!.send('Profiler.stop')) as { profile: Profile }).profile;
        if (process.env.FF_PERF_DUMP_PROFILE) writeFileSync(`${process.env.FF_PERF_DUMP_PROFILE}-${eventsPerSecond}.json`, JSON.stringify(profile));
        const byId = new Map(profile.nodes.map((n) => [n.id, n]));
        let gc = 0;
        profile.samples.forEach((id, i) => {
          if (byId.get(id)!.callFrame.functionName === '(garbage collector)') gc += profile.timeDeltas[i] / 1000;
        });
        windows[name] = { busyPerSecond: busy, scriptPerSecond: script, layoutStylePerSecond: layoutStyle, gcMsPerSecond: round(gc / WINDOW_SECONDS), eventsPerSecond, top: PROFILE ? topFunctions(profile, 14) : [] };
      };
      await sample('list, no analysis', 0);
      await startAnalysis(page, ids, 1);
      await page.waitForTimeout(2000);
      await sample('list, analysis 1 event/s', 1);
      await startAnalysis(page, ids, 6);
      await page.waitForTimeout(2000);
      await sample('list, analysis 6 events/s', 6);
      await stopAnalysis(page);
      report('analysis-list', run, windows);
      await context.close();
    }
  });

  test('memory: heap after open, after a full scroll, idle with analysis', async ({ browser }) => {
    test.skip(!PARTS.has('memory') || WEBKIT);
    const payload = state();
    const ids = payload.tracks.map((t) => t.id);
    const { context, page, cdp } = await openApp(browser, payload);
    await boot(page, cdp);
    await page.waitForTimeout(6000);
    const samples: Record<string, unknown>[] = [{ at: 'after open', ...(await heap(page, cdp)) }];
    await scrollFling(page, 90, 'raf');
    await scrollFling(page, 90, 'raf');
    await page.waitForTimeout(1500);
    samples.push({ at: 'after full scroll x2', ...(await heap(page, cdp)) });
    await page.evaluate(() => ((document.querySelector('.ff-table__scroll') as HTMLElement).scrollTop = 0));
    await startAnalysis(page, ids, 1);
    let last = await counters(cdp);
    for (let minute = 1; minute <= SOAK_MINUTES; minute++) {
      await page.waitForTimeout(60_000);
      const now = await counters(cdp);
      samples.push({ at: `idle with analysis, minute ${minute}`, busyMsPerSecond: round((now.task - last.task) / 60), ...(await heap(page, cdp)) });
      last = now;
    }
    report('memory', 0, samples);
    await context.close();
  });
});
