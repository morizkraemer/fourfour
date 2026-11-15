import { appendFileSync, writeFileSync } from 'node:fs';
import { test as base, expect, type Page } from '@playwright/test';
import type { LoadedState, TrackInfo } from '../src/services/tauri.svelte.ts';
import { mockAnalysisData } from '../src/mocks/analysis.ts';
import { fixturePlaylists, fixtureTracks } from './fixture.ts';
import { installTauriMock, type SmokeWindow } from './tauri-mock.ts';

const TRACKS = Number(process.env.FF_PERF_TRACKS ?? 0);
const FAST_RATE = Number(process.env.FF_PERF_FAST_RATE ?? 6);
const GRID_RATE = Number(process.env.FF_PERF_GRID_RATE ?? 0.2);
const WINDOW_MS = Number(process.env.FF_PERF_WINDOW_MS ?? 6000);
const REPORT = process.env.FF_PERF_OUT;
const SCENARIO = process.env.FF_PERF_SCENARIO ?? 'list';
const PLAYER_TRACK = 3;
const UI_BUSY_FILE = process.env.FF_PERF_UI_BUSY_FILE;

const test = base.extend<{ page: Page }>({});

const durationOf = (trackId: number) => 180 + ((trackId * 7) % 300);

function freshImportState(analysed: number[] = []): LoadedState {
  const tracks: TrackInfo[] = fixtureTracks().map((t) =>
    analysed.includes(t.id)
      ? { ...t, duration_secs: durationOf(t.id), tempo: 12400, key: '8A' }
      : { ...t, duration_secs: durationOf(t.id), tempo: 0, key: '', has_analysis: false, has_cues: false },
  );
  return { tracks, playlists: fixturePlaylists() };
}

interface Bench {
  longTasks: { start: number; duration: number }[];
  emitted: number;
  emitter: number | null;
  cursor: number;
}

const results: Record<string, Record<string, number>> = {};
let readMetrics: (() => Promise<Record<string, number>>) | null = null;

async function mainThreadMs(page: Page) {
  if (!readMetrics) {
    const cdp = await page.context().newCDPSession(page);
    await cdp.send('Performance.enable');
    readMetrics = async () =>
      Object.fromEntries(((await cdp.send('Performance.getMetrics')) as { metrics: { name: string; value: number }[] }).metrics.map((m) => [m.name, m.value * 1000]));
  }
  const m = await readMetrics();
  return { task: m.TaskDuration, script: m.ScriptDuration };
}

async function busyDuring<T>(page: Page, run: () => Promise<T>): Promise<[T, { taskMsPerSec: number; scriptMsPerSec: number }]> {
  const before = await mainThreadMs(page);
  const t0 = Date.now();
  const value = await run();
  const after = await mainThreadMs(page);
  const seconds = (Date.now() - t0) / 1000;
  return [value, { taskMsPerSec: Math.round((after.task - before.task) / seconds), scriptMsPerSec: Math.round((after.script - before.script) / seconds) }];
}

async function answerAnalysisInPage(page: Page) {
  if (process.env.FF_PERF_BINDING) return;
  await page.evaluate(async (template) => {
    const w = window as unknown as SmokeWindow;
    const json = JSON.stringify(await w.__smokeAnalysis(template));
    w.__smokeAnalysis = async (trackId: number) => ({ ...JSON.parse(json), track_id: trackId });
  }, PLAYER_TRACK);
}

async function installBench(page: Page) {
  await page.evaluate(() => {
    const w = window as unknown as { __bulk: Bench };
    w.__bulk = { longTasks: [], emitted: 0, emitter: null, cursor: 0 };
    new PerformanceObserver((list) => {
      for (const e of list.getEntries()) w.__bulk.longTasks.push({ start: e.startTime, duration: e.duration });
    }).observe({ type: 'longtask' });
  });
}

type Order = 'sequential' | 'scattered';

async function startEvents(page: Page, kind: 'fast' | 'grid' | 'none', rate: number, total: number, order: Order = 'sequential') {
  await page.evaluate(
    ({ kind, rate, total, order }) => {
      const pick = (n: number) => (order === 'scattered' ? ((n * 7919) % total) + 1 : (n % total) + 1);
      const w = window as unknown as { __bulk: Bench } & SmokeWindow;
      if (w.__bulk.emitter !== null) clearInterval(w.__bulk.emitter);
      w.__bulk.emitter = null;
      if (kind === 'none') return;
      w.__bulk.emitter = window.setInterval(() => {
        const n = w.__bulk.cursor++;
        if (kind === 'fast') {
          w.__smokeEmit('analysis-partial', { track_id: pick(Math.floor(n / 2)) });
        } else {
          const id = pick(n);
          w.__smokeEmit('analysis-progress', { current: id, total, message: `Analyzed Smoke Track ${id}.mp3 (${id}/${total})`, track_id: id });
        }
        w.__bulk.emitted++;
      }, 1000 / rate);
    },
    { kind, rate, total, order },
  );
}

async function windowStats(page: Page, since: number, emittedBefore: number, fetchesBefore: number) {
  return page.evaluate(
    ({ since, emittedBefore, fetchesBefore }) => {
      const w = window as unknown as { __bulk: Bench } & SmokeWindow;
      const tasks = w.__bulk.longTasks.filter((t) => t.start >= since);
      return {
        events: w.__bulk.emitted - emittedBefore,
        fetches: w.__smokeInvokes.filter((i) => i.cmd === 'get_analysis_data').length - fetchesBefore,
        uiBusySignalsSoFar: w.__smokeInvokes.filter((i) => i.cmd === 'analysis_ui_busy').length,
        longTasks: tasks.length,
        longTaskMsTotal: Math.round(tasks.reduce((a, t) => a + t.duration, 0)),
        longTaskMsMax: Math.round(Math.max(0, ...tasks.map((t) => t.duration))),
      };
    },
    { since, emittedBefore, fetchesBefore },
  );
}

const counters = (page: Page) =>
  page.evaluate(() => {
    const w = window as unknown as { __bulk: Bench } & SmokeWindow;
    return {
      now: performance.now(),
      emitted: w.__bulk.emitted,
      fetches: w.__smokeInvokes.filter((i) => i.cmd === 'get_analysis_data').length,
    };
  });

function scrollFor(page: Page, ms: number) {
  return page.evaluate(
    (ms) =>
      new Promise<{ deltas: number[]; rowsMoved: number; scrollTopOff: number }>((resolve) => {
        const el = document.querySelector('.ff-table__scroll') as HTMLElement;
        const deltas: number[] = [];
        const start = performance.now();
        let last = start;
        let step = 45;
        let expected = el.scrollTop;
        let scrollTopOff = 0;
        let rowsMoved = 0;
        let shown = new Map<string, string>();
        const tick = () => {
          const t = performance.now();
          deltas.push(t - last);
          last = t;
          if (Math.abs(el.scrollTop - expected) > 1) scrollTopOff++;
          const now = new Map<string, string>();
          for (const row of document.querySelectorAll<HTMLElement>('[data-track-row]')) {
            const index = row.dataset.rowIndex!;
            const title = row.querySelector('[data-col-key="title"]')?.textContent ?? '';
            now.set(index, title);
            const before = shown.get(index);
            if (before !== undefined && before !== title) rowsMoved++;
          }
          shown = now;
          if (el.scrollTop + el.clientHeight >= el.scrollHeight - step || (step < 0 && el.scrollTop <= -step)) step = -step;
          el.scrollTop += step;
          expected = el.scrollTop;
          if (t - start < ms) requestAnimationFrame(tick);
          else resolve({ deltas: deltas.slice(1), rowsMoved, scrollTopOff });
        };
        requestAnimationFrame(tick);
      }),
    ms,
  );
}

function holdFor(page: Page, ms: number) {
  return page.evaluate(
    (ms) =>
      new Promise<{ deltas: number[]; rowsShifted: number; rowAtTopChanges: number }>((resolve) => {
        const el = document.querySelector('.ff-table__scroll') as HTMLElement;
        el.scrollTop = Math.floor(el.scrollHeight / 2);
        const deltas: number[] = [];
        const start = performance.now();
        let last = start;
        let rowsShifted = 0;
        let rowAtTopChanges = 0;
        let onScreen = new Map<string, number>();
        let topTitle: string | null = null;
        const tick = () => {
          const t = performance.now();
          deltas.push(t - last);
          last = t;
          const view = el.getBoundingClientRect();
          const probeY = view.top + 60;
          const now = new Map<string, number>();
          let top: string | null = null;
          for (const row of document.querySelectorAll<HTMLElement>('[data-track-row]')) {
            const box = row.getBoundingClientRect();
            if (box.bottom <= view.top || box.top >= view.bottom) continue;
            const title = row.querySelector('[data-col-key="title"]')?.textContent ?? '';
            now.set(title, box.top);
            const before = onScreen.get(title);
            if (before !== undefined && Math.abs(before - box.top) > 1) rowsShifted++;
            if (box.top <= probeY && box.bottom > probeY) top = title;
          }
          onScreen = now;
          if (topTitle !== null && top !== topTitle) rowAtTopChanges++;
          topTitle = top;
          if (t - start < ms) requestAnimationFrame(tick);
          else resolve({ deltas: deltas.slice(1), rowsShifted, rowAtTopChanges });
        };
        requestAnimationFrame(() => requestAnimationFrame(tick));
      }),
    ms,
  );
}

const pct = (values: number[], p: number) => [...values].sort((a, b) => a - b)[Math.min(values.length - 1, Math.floor((p / 100) * values.length))];
const round = (n: number) => Math.round(n * 10) / 10;

function frameStats(deltas: number[]) {
  const interval = 1000 / 60;
  return {
    frames: deltas.length,
    frameP50: round(pct(deltas, 50)),
    frameP95: round(pct(deltas, 95)),
    frameMax: round(Math.max(...deltas)),
    dropped: deltas.filter((d) => d > interval * 1.5).length,
    framesLost: deltas.reduce((a, d) => a + Math.max(0, Math.round(d / interval) - 1), 0),
  };
}

type Profile = { nodes: { id: number; children?: number[]; callFrame: { functionName: string; url: string; lineNumber: number } }[]; samples: number[]; timeDeltas: number[] };

async function profiled<T>(page: Page, name: string, run: () => Promise<T>): Promise<T> {
  if (process.env.FF_PERF_PROFILE !== name) return run();
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Profiler.enable');
  await cdp.send('Profiler.start');
  const value = await run();
  const { profile } = (await cdp.send('Profiler.stop')) as { profile: Profile };
  const parent = new Map<number, number>();
  for (const n of profile.nodes) for (const c of n.children ?? []) parent.set(c, n.id);
  const byId = new Map(profile.nodes.map((n) => [n.id, n]));
  const inclusive = new Map<string, number>();
  profile.samples.forEach((id, i) => {
    const seen = new Set<string>();
    for (let at: number | undefined = id; at !== undefined; at = parent.get(at)) {
      const f = byId.get(at)!.callFrame;
      const key = `${f.functionName || '(anon)'} ${f.url.split('/').pop()}:${f.lineNumber}`;
      if (seen.has(key)) continue;
      seen.add(key);
      inclusive.set(key, (inclusive.get(key) ?? 0) + profile.timeDeltas[i] / 1000);
    }
  });
  console.log(`profile ${name} inclusive`);
  for (const [k, v] of [...inclusive.entries()].sort((a, b) => b[1] - a[1]).slice(0, Number(process.env.FF_PERF_TOP ?? 40))) console.log(`${v.toFixed(0).padStart(7)} ms  ${k}`);
  await cdp.detach();
  return value;
}

type TraceEvent = { name: string; ph: string; dur?: number; tid: number; pid: number; ts: number; args?: { data?: { frame?: string } } };

async function traced<T>(page: Page, name: string, run: () => Promise<T>): Promise<T> {
  if (process.env.FF_PERF_TRACE !== name) return run();
  const cdp = await page.context().newCDPSession(page);
  const events: TraceEvent[] = [];
  cdp.on('Tracing.dataCollected', (e: { value: TraceEvent[] }) => events.push(...e.value));
  const done = new Promise<void>((resolve) => cdp.once('Tracing.tracingComplete', () => resolve()));
  await cdp.send('Tracing.start', { categories: 'devtools.timeline,disabled-by-default-devtools.timeline', transferMode: 'ReportEvents' });
  const value = await run();
  await cdp.send('Tracing.end');
  await done;
  const main = events.find((e) => e.name === 'thread_name' && (e as unknown as { args: { name: string } }).args.name === 'CrRendererMain');
  const byName = new Map<string, { ms: number; count: number; max: number }>();
  for (const e of events) {
    if (!main || e.tid !== main.tid || e.pid !== main.pid || e.ph !== 'X' || !e.dur) continue;
    const entry = byName.get(e.name) ?? { ms: 0, count: 0, max: 0 };
    entry.ms += e.dur / 1000;
    entry.count++;
    entry.max = Math.max(entry.max, e.dur / 1000);
    byName.set(e.name, entry);
  }
  console.log(`trace ${name} (main thread, inclusive)`);
  for (const [k, v] of [...byName.entries()].sort((a, b) => b[1].ms - a[1].ms).slice(0, Number(process.env.FF_PERF_TOP ?? 30)))
    console.log(`${v.ms.toFixed(0).padStart(7)} ms  ${String(v.count).padStart(6)}x  max ${v.max.toFixed(1).padStart(6)}  ${k}`);
  const mainEvents = events.filter((e) => main && e.tid === main.tid && e.pid === main.pid && e.ph === 'X' && e.dur);
  for (const task of mainEvents.filter((e) => e.name === 'RunTask' && e.dur! > 25000)) {
    const inner = new Map<string, number>();
    for (const e of mainEvents) {
      if (e === task || e.ts < task.ts || e.ts + e.dur! > task.ts + task.dur!) continue;
      inner.set(e.name, (inner.get(e.name) ?? 0) + e.dur! / 1000);
    }
    console.log(`long task ${(task.dur! / 1000).toFixed(1)} ms:`, [...inner.entries()].sort((a, b) => b[1] - a[1]).slice(0, 8).map(([k, v]) => `${k} ${v.toFixed(1)}`).join(', '));
  }
  await cdp.detach();
  return value;
}

async function scrollWindow(page: Page, name: string, kind: 'fast' | 'grid' | 'none', rate: number, order: Order = 'sequential') {
  const before = await counters(page);
  await startEvents(page, kind, rate, TRACKS, order);
  const [{ deltas, rowsMoved, scrollTopOff }, busy] = await busyDuring(page, () => traced(page, name, () => profiled(page, name, () => scrollFor(page, WINDOW_MS))));
  await startEvents(page, 'none', 0, TRACKS);
  results[name] = { ...frameStats(deltas), rowsMoved, scrollTopOff, ...busy, ...(await windowStats(page, before.now, before.emitted, before.fetches)) };
}

async function holdWindow(page: Page, name: string, kind: 'fast' | 'grid', rate: number) {
  const before = await counters(page);
  await startEvents(page, kind, rate, TRACKS, 'scattered');
  const [{ deltas, ...stability }, busy] = await busyDuring(page, () => holdFor(page, WINDOW_MS));
  await startEvents(page, 'none', 0, TRACKS);
  results[name] = { ...frameStats(deltas), ...stability, ...busy, ...(await windowStats(page, before.now, before.emitted, before.fetches)) };
}

const settle = (page: Page) =>
  page.evaluate(
    () =>
      new Promise<number>((resolve) => {
        const t0 = (window as unknown as { __keyAt: number }).__keyAt;
        setTimeout(() => requestAnimationFrame(() => requestAnimationFrame(() => resolve(performance.now() - t0))), 0);
      }),
  );

async function typingWindow(page: Page, name: string, kind: 'fast' | 'none', rate: number) {
  await page.evaluate(() => {
    document.addEventListener('keydown', () => ((window as unknown as { __keyAt: number }).__keyAt = performance.now()), true);
  });
  const before = await counters(page);
  await startEvents(page, kind, rate, TRACKS);
  const search = page.locator('.ff-gheader__search input');
  await search.click();
  const keys: number[] = [];
  for (const ch of 'smoke track 12') {
    await page.keyboard.press(ch === ' ' ? 'Space' : ch);
    keys.push(await settle(page));
    await page.waitForTimeout(120);
  }
  const palette: number[] = [];
  await search.fill('');
  await page.keyboard.press('Escape');
  for (let i = 0; i < 3; i++) {
    await page.keyboard.press('Control+k');
    await expect(page.locator('.ff-cp')).toBeVisible();
    palette.push(await settle(page));
    await page.keyboard.press('Escape');
    await expect(page.locator('.ff-cp')).toHaveCount(0);
    await page.waitForTimeout(200);
  }
  await startEvents(page, 'none', 0, TRACKS);
  results[name] = {
    keyMedian: round(pct(keys, 50)),
    keyWorst: round(Math.max(...keys)),
    paletteMedian: round(pct(palette, 50)),
    paletteWorst: round(Math.max(...palette)),
    ...(await windowStats(page, before.now, before.emitted, before.fetches)),
  };
}

test.describe('bulk analysis responsiveness', () => {
  test.skip(!process.env.FF_PERF_BULK || TRACKS === 0 || SCENARIO !== 'list', 'set FF_PERF_BULK=1 FF_PERF_TRACKS=2000 to run');
  test.setTimeout(300_000);

  test('measure', async ({ page }) => {
    await page.exposeFunction('__smokeAnalysis', (trackId: number) => mockAnalysisData(trackId, durationOf(trackId), 12400, '8A'));
    if (UI_BUSY_FILE) await page.exposeFunction('__ffUiBusy', () => appendFileSync(UI_BUSY_FILE, 'b'));
    await page.addInitScript(installTauriMock, freshImportState());
    await page.goto('/');
    await page.locator('[data-track-row]').first().waitFor();
    await page.waitForTimeout(1500);
    await installBench(page);
    await answerAnalysisInPage(page);

    await page.locator('[data-track-row]').nth(2).dblclick();
    await page.waitForTimeout(500);

    await scrollFor(page, 3000);
    await scrollWindow(page, 'idle scroll', 'none', 0);

    await page.keyboard.press('Control+k');
    await page.keyboard.type('Analyze all unanalyzed');
    await page.keyboard.press('Enter');
    await expect(page.locator('.ff-cp')).toHaveCount(0);
    await page.locator('.ff-gheader__search input').fill('');
    await page.keyboard.press('Escape');
    await expect(page.locator('.ff-gheader__count')).toHaveText(TRACKS.toLocaleString('en-US'));
    await page.waitForTimeout(1000);

    const [, still] = await busyDuring(page, () => page.waitForTimeout(3000));
    results['analysing, no events, no input'] = still;
    await scrollWindow(page, 'scroll during fast pass', 'fast', FAST_RATE);
    await scrollWindow(page, 'scroll during grid pass', 'grid', GRID_RATE);
    await typingWindow(page, 'typing + palette during fast pass', 'fast', FAST_RATE);

    await page.locator('.ff-colh__cell[data-col-key="bpm"] .ff-colh__label').click();
    await page.waitForTimeout(500);
    await holdWindow(page, 'held still, BPM sort, fast pass', 'fast', FAST_RATE);
    await holdWindow(page, 'held still, BPM sort, grid pass', 'grid', 2);
    await scrollWindow(page, 'scroll during fast pass, BPM sort', 'fast', FAST_RATE, 'scattered');

    console.log(JSON.stringify({ tracks: TRACKS, fastRate: FAST_RATE, gridRate: GRID_RATE, results }, null, 2));
    if (REPORT) writeFileSync(REPORT, JSON.stringify({ tracks: TRACKS, fastRate: FAST_RATE, gridRate: GRID_RATE, results }, null, 2));
  });
});

function installFrameCost() {
  const w = window as unknown as { __frameCost: Map<number, number> };
  w.__frameCost = new Map();
  const raf = window.requestAnimationFrame.bind(window);
  window.requestAnimationFrame = (cb: FrameRequestCallback) =>
    raf((ts) => {
      const t0 = performance.now();
      try {
        cb(ts);
      } finally {
        w.__frameCost.set(ts, (w.__frameCost.get(ts) ?? 0) + performance.now() - t0);
      }
    });
}

function watchWaveform(page: Page, ms: number) {
  return page.evaluate(
    (ms) =>
      new Promise<{ deltas: number[]; costs: number[] }>((resolve) => {
        const w = window as unknown as { __frameCost: Map<number, number> };
        const deltas: number[] = [];
        const stamps: number[] = [];
        const start = performance.now();
        let last = start;
        const tick = (ts: number) => {
          const t = performance.now();
          deltas.push(t - last);
          stamps.push(ts);
          last = t;
          if (t - start < ms) requestAnimationFrame(tick);
          else resolve({ deltas: deltas.slice(1), costs: stamps.slice(1).map((s) => w.__frameCost.get(s) ?? 0) });
        };
        requestAnimationFrame(tick);
      }),
    ms,
  );
}

async function playerWindow(page: Page, name: string, kind: 'fast' | 'grid' | 'none', rate: number) {
  const before = await counters(page);
  await page.evaluate(() => (window as unknown as { __frameCost: Map<number, number> }).__frameCost.clear());
  await startEvents(page, kind, rate, TRACKS);
  const [{ deltas, costs }, busy] = await busyDuring(page, () => traced(page, name, () => profiled(page, name, () => watchWaveform(page, WINDOW_MS))));
  await startEvents(page, 'none', 0, TRACKS);
  results[name] = {
    ...frameStats(deltas),
    frameP99: round(pct(deltas, 99)),
    rafCostP50: round(pct(costs, 50)),
    rafCostP95: round(pct(costs, 95)),
    rafCostMax: round(Math.max(...costs)),
    ...busy,
    ...(await windowStats(page, before.now, before.emitted, before.fetches)),
  };
}

test.describe('player waveform during bulk analysis', () => {
  test.skip(!process.env.FF_PERF_BULK || TRACKS === 0 || SCENARIO !== 'player', 'set FF_PERF_BULK=1 FF_PERF_SCENARIO=player FF_PERF_TRACKS=10000 to run');
  test.setTimeout(300_000);

  test('measure', async ({ page }) => {
    await page.exposeFunction('__smokeAnalysis', (trackId: number) => mockAnalysisData(trackId, durationOf(trackId), 12400, '8A'));
    await page.addInitScript(installTauriMock, freshImportState([PLAYER_TRACK]));
    await page.addInitScript(() => ((window as unknown as SmokeWindow).__smokeDetail = 'serve'));
    await page.addInitScript(installFrameCost);
    await page.goto('/');
    await page.locator('[data-track-row]').first().waitFor();
    await page.waitForTimeout(1500);
    await installBench(page);
    await answerAnalysisInPage(page);

    const title = `Smoke Track ${String(PLAYER_TRACK).padStart(2, '0')}`;
    await page.locator('.ff-gheader__search input').fill(title);
    await page.keyboard.press('Escape');
    await expect(page.locator('.ff-cp')).toHaveCount(0);
    await page.locator('[data-track-row]').filter({ hasText: title }).first().dblclick();
    await expect(page.locator('.ff-player-compact__meta-title')).toHaveText(title);
    await page.locator('.ff-gheader__search input').fill('');
    await page.keyboard.press('Escape');
    await expect(page.locator('.ff-cp')).toHaveCount(0);
    await expect(page.locator('.ff-gheader__count')).toHaveText(TRACKS.toLocaleString('en-US'));
    await page.locator('.ff-player-compact__expand-btn').click();
    await expect(page.locator('.ff-xp .waveform-display__zoom')).toHaveAttribute('data-bands', /detail|columns/);
    if ((await page.locator('.ff-xp__play').getAttribute('aria-label')) === 'Play') await page.locator('.ff-xp__play').click();
    await expect(page.locator('.ff-xp__play')).toHaveAttribute('aria-label', 'Pause');
    await page.waitForTimeout(1000);
    results.zoomSource = { detail: (await page.locator('.ff-xp .waveform-display__zoom').getAttribute('data-bands')) === 'detail' ? 1 : 0 };

    await watchWaveform(page, 2000);
    await playerWindow(page, 'player, no analysis', 'none', 0);

    await page.keyboard.press('Control+k');
    await page.keyboard.type('Analyze all unanalyzed');
    await page.keyboard.press('Enter');
    await expect(page.locator('.ff-cp')).toHaveCount(0);
    await page.keyboard.press('Escape');
    await page.waitForTimeout(1000);
    await expect(page.locator('.ff-xp__play')).toHaveAttribute('aria-label', 'Pause');

    await playerWindow(page, 'player, analysing, no events', 'none', 0);
    await playerWindow(page, 'player during fast pass', 'fast', FAST_RATE);
    await playerWindow(page, 'player, analysing, no events (2)', 'none', 0);
    await playerWindow(page, 'player during fast pass (2)', 'fast', FAST_RATE);
    await playerWindow(page, 'player during grid pass', 'grid', GRID_RATE);

    console.log(JSON.stringify({ tracks: TRACKS, fastRate: FAST_RATE, results }, null, 2));
    if (REPORT) writeFileSync(REPORT, JSON.stringify({ tracks: TRACKS, fastRate: FAST_RATE, gridRate: GRID_RATE, results }, null, 2));
  });
});
