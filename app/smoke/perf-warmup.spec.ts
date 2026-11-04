import { appendFileSync, readFileSync } from 'node:fs';
import { expect, test } from '@playwright/test';
import type { LoadedState } from '../src/services/tauri.svelte.ts';
import { decodeTrackTable, isTrackTable } from '../src/services/track-table.ts';
import { installTauriMock } from './tauri-mock.ts';

const STATE = process.env.FF_PERF_STATE;
const REPORT = process.env.FF_PERF_OUT;
const LABEL = process.env.FF_PERF_LABEL ?? '';

test.describe('idle work after load', () => {
  test.skip(!STATE || process.env.FF_SMOKE_BROWSER === 'webkit', 'set FF_PERF_STATE=/path/state.json (bench_startup dump); Chromium only (CDP metrics)');
  test.setTimeout(300_000);

  test('script time and long tasks in the seconds after the first rows', async ({ page }) => {
    const dumped = JSON.parse(readFileSync(STATE!, 'utf8'));
    const loaded = { ...dumped, tracks: isTrackTable(dumped.tracks) ? decodeTrackTable(dumped.tracks) : dumped.tracks } as LoadedState;
    await page.addInitScript(installTauriMock, loaded);
    await page.addInitScript(() => {
      const tasks: number[] = [];
      (window as unknown as { __longTasks: number[] }).__longTasks = tasks;
      new PerformanceObserver((list) => list.getEntries().forEach((e) => tasks.push(e.duration))).observe({ type: 'longtask', buffered: true });
    });
    const cdp = await page.context().newCDPSession(page);
    await cdp.send('Performance.enable');
    const script = async () => {
      const m = (await cdp.send('Performance.getMetrics')) as { metrics: { name: string; value: number }[] };
      return m.metrics.find((x) => x.name === 'ScriptDuration')!.value * 1000;
    };
    await page.goto('/');
    await page.locator('[data-track-row]').first().waitFor({ timeout: 120_000 });
    const at = await page.evaluate(() => performance.now());
    const start = await script();
    const startTasks = await page.evaluate(() => (window as unknown as { __longTasks: number[] }).__longTasks.length);
    const samples: number[] = [];
    for (let s = 1; s <= 10; s++) {
      await page.waitForTimeout(1000);
      samples.push((await script()) - start);
    }
    const tasks = await page.evaluate((from) => (window as unknown as { __longTasks: number[] }).__longTasks.slice(from), startTasks);
    expect(samples.length).toBe(10);
    const line = JSON.stringify({ label: LABEL, firstRowsAt: Math.round(at), scriptMsPerSecond: samples.map((v, i) => Math.round(v - (samples[i - 1] ?? 0))), scriptMsTotal: Math.round(samples[9]), longTasks: tasks.map((d) => Math.round(d)) });
    if (REPORT) appendFileSync(REPORT, line + '\n');
    else console.log(line);
  });
});
