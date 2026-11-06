import { createHash } from 'node:crypto';
import { appendFileSync, readFileSync, writeFileSync } from 'node:fs';
import { expect, test, type Page } from '@playwright/test';
import type { LoadedState } from '../src/services/tauri.svelte.ts';
import { decodeTrackTable, isTrackTable } from '../src/services/track-table.ts';
import { mockAnalysisData } from '../src/mocks/analysis.ts';
import type { SmokeWindow } from './tauri-mock.ts';
import { installTauriMock } from './tauri-mock.ts';

const STATE = process.env.FF_PERF_STATE;
const REPORT = process.env.FF_PERF_OUT;
const SIZES = (process.env.FF_PERF_SIZES ?? '2000').split(',').map(Number);
const LABEL = process.env.FF_PERF_LABEL ?? '';

function state(): LoadedState {
  const dumped = JSON.parse(readFileSync(STATE!, 'utf8'));
  const loaded = { ...dumped, tracks: isTrackTable(dumped.tracks) ? decodeTrackTable(dumped.tracks) : dumped.tracks } as LoadedState;
  const bulk = SIZES.map((size, i) => ({
    id: 9001 + i,
    name: `Bulk ${size}`,
    track_ids: loaded.tracks.slice(0, size).map((t) => t.id),
    parent_id: 0,
    is_folder: false,
  }));
  return { ...loaded, playlists: [...loaded.playlists, ...bulk] };
}

interface Bench {
  longTasks: { start: number; duration: number }[];
}

async function metrics(page: Page) {
  if (process.env.FF_SMOKE_BROWSER === 'webkit') return async () => ({ script: 0, task: 0 });
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Performance.enable');
  return async () => {
    const m = Object.fromEntries(((await cdp.send('Performance.getMetrics')) as { metrics: { name: string; value: number }[] }).metrics.map((x) => [x.name, x.value * 1000]));
    return { script: m.ScriptDuration, task: m.TaskDuration };
  };
}

const topRows = (page: Page) =>
  page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>('[data-track-row]')]
      .map((row) => `${row.dataset.rowIndex}:${row.textContent?.replace(/\s+/g, ' ').trim()}`)
      .join('\n'),
  );

const sha = (text: string) => createHash('sha1').update(text).digest('hex').slice(0, 12);

async function fullView(page: Page) {
  const count = (await page.locator('.ff-gheader__count').textContent())?.trim() ?? '';
  const parts: string[] = [count];
  const heights = await page.evaluate(() => {
    const el = document.querySelector('.ff-table__scroll') as HTMLElement;
    return el.scrollHeight - el.clientHeight;
  });
  for (const at of [0, 0.5, 1]) {
    await page.evaluate((top) => ((document.querySelector('.ff-table__scroll') as HTMLElement).scrollTop = top), Math.floor(heights * at));
    await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
    await page.waitForTimeout(80);
    parts.push(await topRows(page));
  }
  await page.evaluate(() => ((document.querySelector('.ff-table__scroll') as HTMLElement).scrollTop = 0));
  await page.waitForTimeout(350);
  return `${count}/${sha(parts.join('\n--\n'))}`;
}

const sidebarRow = (page: Page, name: string) => page.locator('.ff-sbrow__label', { hasText: new RegExp(`^${name}$`) });

async function quiet(page: Page, since: number, ms = 1500, cap = 120_000) {
  const start = Date.now();
  for (;;) {
    await page.waitForTimeout(250);
    const lastEnd = await page.evaluate((s) => {
      const tasks = (window as unknown as { __bulk: Bench }).__bulk.longTasks.filter((t) => t.start >= s);
      return tasks.length ? tasks[tasks.length - 1].start + tasks[tasks.length - 1].duration : s;
    }, since);
    const now = await page.evaluate(() => performance.now());
    if (now - lastEnd > ms) return;
    if (Date.now() - start > cap) return;
  }
}

test.describe('bulk bump', () => {
  test.skip(!STATE, 'set FF_PERF_STATE=/path/state.json (bench_startup dump)');
  test.setTimeout(1_800_000);

  for (const size of SIZES) {
    test(`bulk ${size}`, async ({ page }) => {
      await page.exposeFunction('__smokeAnalysis', (trackId: number) => mockAnalysisData(trackId, 180 + ((trackId * 7) % 300), 12400, '8A'));
      await page.addInitScript(installTauriMock, state());
      await page.goto('/');
      await page.locator('[data-track-row]').first().waitFor({ timeout: 120_000 });
      await page.waitForTimeout(4000);
      await page.evaluate(async () => {
        const w = window as unknown as { __bulk: Bench } & SmokeWindow;
        w.__bulk = { longTasks: [] };
        new PerformanceObserver((list) => {
          for (const e of list.getEntries()) w.__bulk.longTasks.push({ start: e.startTime, duration: e.duration });
        }).observe({ type: 'longtask' });
        const json = JSON.stringify(await w.__smokeAnalysis(3));
        w.__smokeAnalysis = async (trackId: number) => ({ ...JSON.parse(json), track_id: trackId });
      });
      const read = await metrics(page);
      const out: Record<string, unknown> = { label: LABEL, size, browser: process.env.FF_SMOKE_BROWSER ?? 'chromium', steps: [] as unknown[] };
      const steps = out.steps as Record<string, unknown>[];

      const longSince = (since: number) =>
        page.evaluate((s) => {
          const tasks = (window as unknown as { __bulk: Bench }).__bulk.longTasks.filter((t) => t.start >= s);
          return { count: tasks.length, max: Math.round(Math.max(0, ...tasks.map((t) => t.duration))), total: Math.round(tasks.reduce((a, t) => a + t.duration, 0)) };
        }, since);

      await sidebarRow(page, `Bulk ${size}`).click();
      await expect(page.locator('.ff-gheader__count')).toHaveText(Math.min(size, 1e9).toLocaleString('en-US'), { timeout: 60_000 });
      await page.waitForTimeout(1500);

      const clickRow = (which: 'first' | 'last') =>
        page.evaluate((w) => {
          const all = document.querySelectorAll('[data-track-row]');
          const row = (w === 'first' ? all[0] : all[all.length - 1]) as HTMLElement;
          row.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, shiftKey: w === 'last' }));
        }, which);
      await clickRow('first');
      await page.evaluate(() => {
        const el = document.querySelector('.ff-table__scroll') as HTMLElement;
        el.scrollTop = el.scrollHeight;
      });
      await page.waitForTimeout(800);
      await clickRow('last');
      await page.waitForTimeout(500);
      steps.push({ name: 'playlist view before', view: await fullView(page) });

      await page.keyboard.press('Control+k');
      await page.keyboard.type('Analyze selection');
      await page.waitForTimeout(300);
      const analyzeStart = await page.evaluate(() => performance.now());
      const beforeAnalyze = await read();
      const profilerOut = process.env.FF_PERF_PROFILE_JSON;
      const cdp = profilerOut ? await page.context().newCDPSession(page) : null;
      if (cdp) {
        await cdp.send('Profiler.enable');
        await cdp.send('Profiler.setSamplingInterval', { interval: 200 });
        await cdp.send('Profiler.start');
      }
      await page.keyboard.press('Enter');
      await quiet(page, analyzeStart, 1200);
      if (cdp) writeFileSync(profilerOut!, JSON.stringify(((await cdp.send('Profiler.stop')) as { profile: unknown }).profile));
      const afterAnalyze = await read();
      steps.push({
        name: 'analyze selection',
        ...(await longSince(analyzeStart)),
        taskMs: Math.round(afterAnalyze.task - beforeAnalyze.task),
        scriptMsExact: Math.round(afterAnalyze.script - beforeAnalyze.script),
      });
      await expect(page.locator('.ff-cp')).toHaveCount(0);
      steps.push({ name: 'playlist view after analyze', view: await fullView(page) });
      await sidebarRow(page, 'All Tracks').click();
      await page.waitForTimeout(1500);
      steps.push({ name: 'all tracks view after analyze', view: await fullView(page) });

      if (process.env.FF_PERF_ARRIVE !== '0') {
        const arriveStart = await page.evaluate(() => performance.now());
        const beforeArrive = await read();
        const arriveProfileOut = process.env.FF_PERF_ARRIVE_PROFILE_JSON;
        const arriveCdp = arriveProfileOut ? await page.context().newCDPSession(page) : null;
        if (arriveCdp) {
          await arriveCdp.send('Profiler.enable');
          await arriveCdp.send('Profiler.setSamplingInterval', { interval: 200 });
          await arriveCdp.send('Profiler.start');
        }
        await page.evaluate((n) => {
          const w = window as unknown as SmokeWindow;
          for (let id = 1; id <= n; id++) w.__smokeEmit('analysis-partial', { track_id: id });
        }, size);
        await quiet(page, arriveStart, 2500);
        if (arriveCdp) writeFileSync(arriveProfileOut!, JSON.stringify(((await arriveCdp.send('Profiler.stop')) as { profile: unknown }).profile));
        const afterArrive = await read();
        steps.push({
          name: 'analysis results arrive (events for every row)',
          ...(await longSince(arriveStart)),
          taskMs: Math.round(afterArrive.task - beforeArrive.task),
          scriptMsExact: Math.round(afterArrive.script - beforeArrive.script),
        });
        steps.push({ name: 'all tracks view after results', view: await fullView(page) });
        await sidebarRow(page, `Bulk ${size}`).click();
        await page.waitForTimeout(1500);
        steps.push({ name: 'playlist view after results', view: await fullView(page) });
      }

      const line = JSON.stringify(out);
      if (REPORT) appendFileSync(REPORT, line + '\n');
      else console.log(line);
    });
  }
});
