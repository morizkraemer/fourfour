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
const SORTS = (process.env.FF_PERF_SORTS ?? 'title,artist,bpm').split(',');
const ANALYSIS = process.env.FF_PERF_ANALYSIS === '1';
const FAST_RATE = Number(process.env.FF_PERF_FAST_RATE ?? 6);
const AGE_SECONDS = Number(process.env.FF_PERF_AGE ?? 10);
const WEBKIT = process.env.FF_SMOKE_BROWSER === 'webkit';

function state(): LoadedState {
  const dumped = JSON.parse(readFileSync(STATE!, 'utf8'));
  return { ...dumped, tracks: isTrackTable(dumped.tracks) ? decodeTrackTable(dumped.tracks) : dumped.tracks } as LoadedState;
}

async function metrics(page: Page) {
  if (WEBKIT) return async () => 0;
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Performance.enable');
  return async () => {
    const m = (await cdp.send('Performance.getMetrics')) as { metrics: { name: string; value: number }[] };
    return m.metrics.find((x) => x.name === 'ScriptDuration')!.value * 1000;
  };
}

const settle = (page: Page) =>
  page.evaluate(
    () =>
      new Promise<{ total: number; js: number }>((resolve) => {
        const w = window as unknown as { __inputAt: number; __jsAt: number };
        setTimeout(() => requestAnimationFrame(() => requestAnimationFrame(() => resolve({ total: performance.now() - w.__inputAt, js: w.__jsAt - w.__inputAt }))), 0);
      }),
  );

const visibleRows = (page: Page) =>
  page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>('[data-track-row]')]
      .map((row) => `${row.dataset.rowIndex}:${row.textContent?.replace(/\s+/g, ' ').trim()}`)
      .join('\n'),
  );

const sha = (text: string) => createHash('sha1').update(text).digest('hex').slice(0, 12);

test.describe('typing in a sorted view', () => {
  test.skip(!STATE, 'set FF_PERF_STATE=/path/state.json (bench_startup dump)');
  test.setTimeout(600_000);

  for (let run = 0; run < RUNS; run++) {
    test(`run ${run}`, async ({ page }) => {
      await page.exposeFunction('__smokeAnalysis', (trackId: number) => mockAnalysisData(trackId, 180 + ((trackId * 7) % 300), 12400, '8A'));
      await page.addInitScript(installTauriMock, state());
      await page.addInitScript(() => {
        const frame = () => requestAnimationFrame(frame);
        frame();
        const w = window as unknown as { __inputAt: number; __jsAt: number };
        for (const type of ['keydown', 'pointerdown']) {
          document.addEventListener(
            type,
            () => {
              w.__inputAt = performance.now();
              setTimeout(() => (w.__jsAt = performance.now()), 0);
            },
            true,
          );
        }
      });
      const script = await metrics(page);
      await page.goto('/');
      await page.locator('[data-track-row]').first().waitFor({ timeout: 120_000 });
      await page.waitForTimeout(8000);
      if (ANALYSIS) {
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
      }
      const steps: { group: string; name: string; total: number; js: number; script: number; view: string }[] = [];
      const profileSort = process.env.FF_PERF_PROFILE_SORT ?? 'title';
      const profiler = process.env.FF_PERF_PROFILE ? await page.context().newCDPSession(page) : null;
      const profileEach = process.env.FF_PERF_PROFILE_EACH;
      const measure = async (group: string, name: string, action: () => Promise<void>, gap = 500) => {
        const each = profiler && profileEach && group === profileSort;
        if (each) {
          await profiler.send('Profiler.enable');
          await profiler.send('Profiler.setSamplingInterval', { interval: 100 });
          await profiler.send('Profiler.start');
        }
        const before = await script();
        await action();
        const { total, js } = await settle(page);
        const used = (await script()) - before;
        if (each) {
          const { profile } = (await profiler.send('Profiler.stop')) as { profile: unknown };
          writeFileSync(`${profileEach}/${steps.length}-${name.replace(/[^a-z0-9]+/gi, '_')}.cpuprofile`, JSON.stringify(profile));
        }
        steps.push({ group, name, total: Math.round(total), js: Math.round(js), script: Math.round(used), view: sha(await visibleRows(page)) });
        await page.waitForTimeout(gap);
      };
      const search = page.locator('.ff-gheader__search input');
      for (const key of SORTS) {
        if (key !== "none") await page.locator(`.ff-colh__cell[data-col-key="${key}"] .ff-colh__label`).click();
        await page.waitForTimeout(800);
        await search.click();
        if (profiler && !profileEach && key === profileSort) {
          await profiler.send('Profiler.enable');
          await profiler.send('Profiler.setSamplingInterval', { interval: 100 });
          await profiler.send('Profiler.start');
        }
        let typed = '';
        for (const ch of 'house') {
          typed += ch;
          await measure(key, `type "${typed}"`, () => page.keyboard.press(ch));
        }
        for (let i = 4; i >= 0; i--) await measure(key, `back to "${'house'.slice(0, i)}"`, () => page.keyboard.press('Backspace'));
        typed = '';
        for (const ch of 'a b') {
          typed += ch;
          await measure(key, `type "${typed}"`, () => page.keyboard.press(ch));
        }
        for (let i = 2; i >= 0; i--) await measure(key, `back to "${'a b'.slice(0, i)}"`, () => page.keyboard.press('Backspace'));
        if (profiler && !profileEach && key === profileSort) {
          const { profile } = (await profiler.send('Profiler.stop')) as { profile: unknown };
          writeFileSync(process.env.FF_PERF_PROFILE_JSON ?? '/tmp/r147.cpuprofile', JSON.stringify(profile));
        }
        await page.keyboard.press('Escape');
      }
      const line = JSON.stringify({ label: LABEL, analysis: ANALYSIS, run, steps });
      if (REPORT) appendFileSync(REPORT, line + '\n');
      else console.log(line);
    });
  }
});
