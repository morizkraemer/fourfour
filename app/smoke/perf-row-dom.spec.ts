import { createHash } from 'node:crypto';
import { appendFileSync, readFileSync } from 'node:fs';
import { test } from '@playwright/test';
import type { LoadedState } from '../src/services/tauri.svelte.ts';
import { decodeTrackTable, isTrackTable } from '../src/services/track-table.ts';
import { mockAnalysisData } from '../src/mocks/analysis.ts';
import { installTauriMock } from './tauri-mock.ts';

const STATE = process.env.FF_PERF_STATE;
const REPORT = process.env.FF_DOM_OUT;
const LABEL = process.env.FF_PERF_LABEL ?? '';
const SORTS = (process.env.FF_PERF_SORTS ?? 'none,title,bpm').split(',');
const QUERIES = ['house', 'hou', 'a b', 'dub', 'x', 'a', 'mix remix'];

function state(): LoadedState {
  const dumped = JSON.parse(readFileSync(STATE!, 'utf8'));
  return { ...dumped, tracks: isTrackTable(dumped.tracks) ? decodeTrackTable(dumped.tracks) : dumped.tracks } as LoadedState;
}

const sha = (text: string) => createHash('sha1').update(text).digest('hex').slice(0, 12);

test.describe('visible rows markup per key', () => {
  test.skip(!STATE, 'set FF_PERF_STATE=/path/state.json');
  test.setTimeout(600_000);

  test('rows', async ({ page }) => {
    await page.exposeFunction('__smokeAnalysis', (trackId: number) => mockAnalysisData(trackId, 180 + ((trackId * 7) % 300), 12400, '8A'));
    await page.addInitScript(installTauriMock, state());
    await page.goto('/');
    await page.locator('[data-track-row]').first().waitFor({ timeout: 120_000 });
    await page.waitForTimeout(4000);
    const rows = () =>
      page.evaluate(() =>
        [...document.querySelectorAll<HTMLElement>('[data-track-row]')]
          .map((row) => row.outerHTML.replace(/<canvas[^>]*>/g, '<canvas>'))
          .join('\n'),
      );
    const canvases = () =>
      page.evaluate(() =>
        [...document.querySelectorAll<HTMLCanvasElement>('[data-track-row] canvas')].map((c) => c.toDataURL().length + ':' + c.toDataURL().slice(-64)).join('|'),
      );
    const out: { step: string; dom: string; canvas: string }[] = [];
    const record = async (step: string) => {
      await page.waitForTimeout(400);
      out.push({ step, dom: sha(await rows()), canvas: sha(await canvases()) });
    };
    const search = page.locator('.ff-gheader__search input');
    for (const key of SORTS) {
      if (key !== 'none') await page.locator(`.ff-colh__cell[data-col-key="${key}"] .ff-colh__label`).click();
      await page.waitForTimeout(600);
      await search.click();
      for (const q of QUERIES) {
        let typed = '';
        for (const ch of q) {
          await page.keyboard.press(ch === ' ' ? 'Space' : ch);
          typed += ch;
          await record(`${key} type "${typed}"`);
        }
        for (let i = q.length - 1; i >= 0; i--) {
          await page.keyboard.press('Backspace');
          await record(`${key} back "${q.slice(0, i)}"`);
        }
      }
      await page.keyboard.press('Escape');
    }
    const line = JSON.stringify({ label: LABEL, out });
    if (REPORT) appendFileSync(REPORT, line + '\n');
    else console.log(line);
  });
});
