import { writeFileSync } from 'node:fs';
import { test as base, expect, type Page } from '@playwright/test';
import type { LoadedState, TrackInfo } from '../src/services/tauri.svelte.ts';
import { mockAnalysisData } from '../src/mocks/analysis.ts';
import { fixturePlaylists, fixtureTracks } from './fixture.ts';
import { installTauriMock, type SmokeWindow } from './tauri-mock.ts';

const TRACKS = Number(process.env.FF_PERF_TRACKS ?? 0);
const FAST_RATE = Number(process.env.FF_PERF_FAST_RATE ?? 12);
const GRID_RATE = Number(process.env.FF_PERF_GRID_RATE ?? 2);
const WINDOW_MS = Number(process.env.FF_PERF_WINDOW_MS ?? 6000);
const REPORT = process.env.FF_PERF_OUT;

const test = base.extend<{ page: Page }>({});

const durationOf = (trackId: number) => 180 + ((trackId * 7) % 300);

function freshImportState(): LoadedState {
  const tracks: TrackInfo[] = fixtureTracks().map((t) => ({ ...t, duration_secs: durationOf(t.id), tempo: 0, key: '', has_analysis: false, has_cues: false }));
  return { tracks, playlists: fixturePlaylists() };
}

interface Bench {
  longTasks: { start: number; duration: number }[];
  emitted: number;
  emitter: number | null;
  cursor: number;
}

const results: Record<string, Record<string, number>> = {};

async function installBench(page: Page) {
  await page.evaluate(() => {
    const w = window as unknown as { __bulk: Bench };
    w.__bulk = { longTasks: [], emitted: 0, emitter: null, cursor: 0 };
    new PerformanceObserver((list) => {
      for (const e of list.getEntries()) w.__bulk.longTasks.push({ start: e.startTime, duration: e.duration });
    }).observe({ type: 'longtask' });
  });
}

async function startEvents(page: Page, kind: 'fast' | 'grid' | 'none', rate: number, total: number) {
  await page.evaluate(
    ({ kind, rate, total }) => {
      const w = window as unknown as { __bulk: Bench } & SmokeWindow;
      if (w.__bulk.emitter !== null) clearInterval(w.__bulk.emitter);
      w.__bulk.emitter = null;
      if (kind === 'none') return;
      w.__bulk.emitter = window.setInterval(() => {
        const n = w.__bulk.cursor++;
        if (kind === 'fast') {
          w.__smokeEmit('analysis-partial', { track_id: (Math.floor(n / 2) % total) + 1 });
        } else {
          const id = (n % total) + 1;
          w.__smokeEmit('analysis-progress', { current: id, total, message: `Analyzed Smoke Track ${id}.mp3 (${id}/${total})`, track_id: id });
        }
        w.__bulk.emitted++;
      }, 1000 / rate);
    },
    { kind, rate, total },
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
      new Promise<number[]>((resolve) => {
        const el = document.querySelector('.ff-table__scroll') as HTMLElement;
        const deltas: number[] = [];
        const start = performance.now();
        let last = start;
        let step = 45;
        const tick = () => {
          const t = performance.now();
          deltas.push(t - last);
          last = t;
          if (el.scrollTop + el.clientHeight >= el.scrollHeight - step || (step < 0 && el.scrollTop <= -step)) step = -step;
          el.scrollTop += step;
          if (t - start < ms) requestAnimationFrame(tick);
          else resolve(deltas.slice(1));
        };
        requestAnimationFrame(tick);
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

async function scrollWindow(page: Page, name: string, kind: 'fast' | 'grid' | 'none', rate: number) {
  const before = await counters(page);
  await startEvents(page, kind, rate, TRACKS);
  const deltas = await scrollFor(page, WINDOW_MS);
  await startEvents(page, 'none', 0, TRACKS);
  results[name] = { ...frameStats(deltas), ...(await windowStats(page, before.now, before.emitted, before.fetches)) };
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
  test.skip(!process.env.FF_PERF_BULK || TRACKS === 0, 'set FF_PERF_BULK=1 FF_PERF_TRACKS=2000 to run');
  test.setTimeout(300_000);

  test('measure', async ({ page }) => {
    await page.exposeFunction('__smokeAnalysis', (trackId: number) => mockAnalysisData(trackId, durationOf(trackId), 12400, '8A'));
    await page.addInitScript(installTauriMock, freshImportState());
    await page.goto('/');
    await page.locator('[data-track-row]').first().waitFor();
    await page.waitForTimeout(1500);
    await installBench(page);

    await page.locator('[data-track-row]').nth(2).dblclick();
    await page.waitForTimeout(500);

    await scrollWindow(page, 'idle scroll', 'none', 0);

    await page.keyboard.press('Control+k');
    await page.keyboard.type('Analyze all unanalyzed');
    await page.keyboard.press('Enter');
    await expect(page.locator('.ff-cp')).toHaveCount(0);
    await page.waitForTimeout(1000);

    await scrollWindow(page, 'scroll during fast pass', 'fast', FAST_RATE);
    await scrollWindow(page, 'scroll during grid pass', 'grid', GRID_RATE);
    await typingWindow(page, 'typing + palette during fast pass', 'fast', FAST_RATE);

    console.log(JSON.stringify({ tracks: TRACKS, fastRate: FAST_RATE, gridRate: GRID_RATE, results }, null, 2));
    if (REPORT) writeFileSync(REPORT, JSON.stringify({ tracks: TRACKS, fastRate: FAST_RATE, gridRate: GRID_RATE, results }, null, 2));
  });
});
