import { createHash } from 'node:crypto';
import { appendFileSync, readFileSync, writeFileSync } from 'node:fs';
import { test, type Page } from '@playwright/test';
import type { LoadedState } from '../src/services/tauri.svelte.ts';
import { decodeTrackTable, isTrackTable } from '../src/services/track-table.ts';
import { mockAnalysisData } from '../src/mocks/analysis.ts';
import type { SmokeWindow } from './tauri-mock.ts';
import { installTauriMock } from './tauri-mock.ts';

const STATE = process.env.FF_PERF_STATE;
const REPORT = process.env.FF_PERF_OUT;
const RUNS = Number(process.env.FF_PERF_RUNS ?? 1);
const LABEL = process.env.FF_PERF_LABEL ?? '';
const MODE = process.env.FF_PERF_MODE ?? 'analysis';
const AGE_SECONDS = Number(process.env.FF_PERF_AGE ?? 20);
const FAST_RATE = Number(process.env.FF_PERF_FAST_RATE ?? 6);
const WEBKIT = process.env.FF_SMOKE_BROWSER === 'webkit';

function state(): LoadedState {
  const dumped = JSON.parse(readFileSync(STATE!, 'utf8'));
  return { ...dumped, tracks: isTrackTable(dumped.tracks) ? decodeTrackTable(dumped.tracks) : dumped.tracks } as LoadedState;
}

async function metrics(page: Page) {
  if (WEBKIT) return async () => 0;
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Performance.enable');
  return async (name = 'ScriptDuration') => {
    const m = (await cdp.send('Performance.getMetrics')) as { metrics: { name: string; value: number }[] };
    return m.metrics.find((x) => x.name === name)!.value * 1000;
  };
}

const settle = (page: Page) =>
  page.evaluate(
    () =>
      new Promise<number>((resolve) => {
        const t0 = (window as unknown as { __inputAt: number }).__inputAt;
        setTimeout(() => requestAnimationFrame(() => requestAnimationFrame(() => resolve(performance.now() - t0))), 0);
      }),
  );

const visibleRows = (page: Page) =>
  page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>('[data-track-row]')]
      .map((row) => `${row.dataset.rowIndex}:${row.textContent?.replace(/\s+/g, ' ').trim()}`)
      .join('\n'),
  );

const sha = (text: string) => createHash('sha1').update(text).digest('hex').slice(0, 12);

test.describe('sort and search while analysis bumps rows', () => {
  test.skip(!STATE, 'set FF_PERF_STATE=/path/state.json (bench_startup dump)');
  test.setTimeout(600_000);

  for (let run = 0; run < RUNS; run++) {
    test(`run ${run}`, async ({ page }) => {
      await page.exposeFunction('__smokeAnalysis', (trackId: number) => mockAnalysisData(trackId, 180 + ((trackId * 7) % 300), 12400, '8A'));
      await page.addInitScript(installTauriMock, state());
      await page.addInitScript(() => {
        const frame = () => requestAnimationFrame(frame);
        frame();
        const w = window as unknown as { __inputAt: number };
        const mark = () => (w.__inputAt = performance.now());
        for (const type of ['keydown', 'pointerdown']) document.addEventListener(type, mark, true);
      });
      const script = await metrics(page);
      await page.goto('/');
      await page.locator('[data-track-row]').first().waitFor({ timeout: 120_000 });
      await page.waitForTimeout(8000);
      if (MODE === 'analysis') {
        await page.evaluate(
          ({ rate, count }) => {
            const w = window as unknown as SmokeWindow & { __emitter?: number; __cursor?: number };
            w.__cursor = 0;
            w.__emitter = window.setInterval(() => {
              const n = w.__cursor!++;
              w.__smokeEmit('analysis-partial', { track_id: ((Math.floor(n / 2) * 7919) % count) + 1 });
            }, 1000 / rate);
          },
          { rate: FAST_RATE, count: 10_000 },
        );
        await page.waitForTimeout(AGE_SECONDS * 1000);
        if (process.env.FF_PERF_STOP) {
          await page.evaluate(() => clearInterval((window as unknown as { __emitter: number }).__emitter));
          await page.waitForTimeout(1500);
        }
      }
      const steps: { name: string; total: number; script: number; view: string }[] = [];
      const measure = async (name: string, action: () => Promise<void>, gap = 600) => {
        const before = await script();
        await action();
        const total = await settle(page);
        const used = (await script()) - before;
        steps.push({ name, total: Math.round(total), script: Math.round(used), view: sha(await visibleRows(page)) });
        await page.waitForTimeout(gap);
      };
      const profiler = process.env.FF_PERF_PROFILE ? await page.context().newCDPSession(page) : null;
      if (profiler) {
        await profiler.send('Profiler.enable');
        await profiler.send('Profiler.setSamplingInterval', { interval: 200 });
        await profiler.send('Profiler.start');
      }
      const sortBy = (key: string, label: string) => measure(`sort ${label}`, () => page.locator(`.ff-colh__cell[data-col-key="${key}"] .ff-colh__label`).click());
      const search = page.locator('.ff-gheader__search input');
      for (const [key, label] of [['artist', 'artist asc'], ['artist', 'artist desc'], ['title', 'title asc'], ['bpm', 'bpm asc'], ['key', 'key asc'], ['time', 'time asc'], ['label', 'label asc']] as const) {
        await sortBy(key, label);
      }
      await search.click();
      await measure("sorted type 'a'", () => page.keyboard.press('a'));
      await measure("sorted type 'm'", () => page.keyboard.press('m'));
      await measure('sorted clear', () => search.fill(''), 800);
      await page.keyboard.press('Escape');
      await sortBy('artist', 'artist asc again');
      await sortBy('title', 'title desc');
      if (profiler) {
        const { profile } = (await profiler.send('Profiler.stop')) as { profile: unknown };
        writeFileSync(process.env.FF_PERF_PROFILE_JSON ?? '/tmp/r137.cpuprofile', JSON.stringify(profile));
      }
      let quiet: { seconds: number; script: number; task: number } | null = null;
      if (MODE === 'analysis' && !process.env.FF_PERF_STOP && process.env.FF_PERF_QUIET) {
        const seconds = Number(process.env.FF_PERF_QUIET);
        const [script0, task0] = [await script(), await script('TaskDuration')];
        await page.waitForTimeout(seconds * 1000);
        quiet = { seconds, script: Math.round((await script()) - script0), task: Math.round((await script('TaskDuration')) - task0) };
      }
      if (MODE === 'analysis' && !process.env.FF_PERF_STOP) await page.evaluate(() => clearInterval((window as unknown as { __emitter: number }).__emitter));
      const line = JSON.stringify({ label: LABEL, mode: MODE, age: AGE_SECONDS, run, quiet, steps });
      if (REPORT) appendFileSync(REPORT, line + '\n');
      else console.log(line);
    });
  }
});
