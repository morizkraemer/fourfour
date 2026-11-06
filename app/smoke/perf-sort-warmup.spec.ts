import { appendFileSync, readFileSync } from 'node:fs';
import { test, type Page } from '@playwright/test';
import type { LoadedState } from '../src/services/tauri.svelte.ts';
import { decodeTrackTable, isTrackTable } from '../src/services/track-table.ts';
import { installTauriMock } from './tauri-mock.ts';

const STATE = process.env.FF_PERF_STATE;
const REPORT = process.env.FF_PERF_OUT;
const RUNS = Number(process.env.FF_PERF_RUNS ?? 1);
const LABEL = process.env.FF_PERF_LABEL ?? '';
const KEY_AT_MS = Number(process.env.FF_PERF_KEY_AT ?? 1500);
const SORT_AT_MS = Number(process.env.FF_PERF_SORT_AT ?? 8000);
const MODE = process.env.FF_PERF_MODE ?? 'key';
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
      new Promise<number>((resolve) => {
        const t0 = (window as unknown as { __inputAt: number }).__inputAt;
        setTimeout(() => requestAnimationFrame(() => requestAnimationFrame(() => resolve(performance.now() - t0))), 0);
      }),
  );

test.describe('sort warm-up cost and the first interactions', () => {
  test.skip(!STATE, 'set FF_PERF_STATE=/path/state.json (bench_startup dump)');
  test.setTimeout(300_000);

  for (let run = 0; run < RUNS; run++) {
    test(`run ${run}`, async ({ page }) => {
      await page.addInitScript(installTauriMock, state());
      if (MODE !== 'warm') await page.addInitScript(() => {
        const frame = () => requestAnimationFrame(frame);
        frame();
      });
      await page.addInitScript(() => {
        const w = window as unknown as { __inputAt: number };
        const mark = () => (w.__inputAt = performance.now());
        for (const type of ['keydown', 'pointerdown']) document.addEventListener(type, mark, true);
      });
      const script = await metrics(page);
      await page.goto('/');
      await page.locator('[data-track-row]').first().waitFor({ timeout: 120_000 });
      const scriptAtRows = await script();
      const result: Record<string, number | string> = { label: LABEL, mode: MODE, run };
      if (MODE === 'warm') {
        await page.waitForTimeout(3000);
        result.warmupScriptMsFirst3s = Math.round((await script()) - scriptAtRows);
        await page.waitForTimeout(7000);
        result.warmupScriptMsFirst10s = Math.round((await script()) - scriptAtRows);
      } else if (MODE === 'key') {
        await page.locator('.ff-gheader__search input').click();
        await page.waitForTimeout(KEY_AT_MS);
        const before = await script();
        await page.keyboard.press('g');
        result.firstKeyAtMs = KEY_AT_MS;
        result.firstKeyTotalMs = Math.round(await settle(page));
        result.firstKeyScriptMs = Math.round((await script()) - before);
      } else {
        await page.waitForTimeout(SORT_AT_MS);
        const before = await script();
        await page.locator('.ff-colh__cell[data-col-key="title"] .ff-colh__label').click();
        result.firstSortAtMs = SORT_AT_MS;
        result.firstSortTotalMs = Math.round(await settle(page));
        result.firstSortScriptMs = Math.round((await script()) - before);
      }
      const line = JSON.stringify(result);
      if (REPORT) appendFileSync(REPORT, line + '\n');
      else console.log(line);
    });
  }
});
