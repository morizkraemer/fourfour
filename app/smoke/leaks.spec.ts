import type { CDPSession, Page } from '@playwright/test';
import { expect, loadByDoubleClick, test } from './app.ts';
import type { SmokeWindow } from './tauri-mock.ts';

const CYCLES = 6;
const CDP_COUNTERS = process.env.FF_SMOKE_BROWSER !== 'webkit';

interface Counters {
  listeners: number;
  nodes: number;
  callbacks: number;
}

async function counters(page: Page, cdp: CDPSession | null): Promise<Counters> {
  const callbacks = await page.evaluate(() => Object.keys(window).filter((key) => /^_\d+$/.test(key)).length);
  if (!cdp) return { listeners: 0, nodes: 0, callbacks };
  await cdp.send('HeapProfiler.collectGarbage');
  const dom = (await cdp.send('Memory.getDOMCounters')) as { nodes: number; jsEventListeners: number };
  return { listeners: dom.jsEventListeners, nodes: dom.nodes, callbacks };
}

async function settledCounters(page: Page, cdp: CDPSession | null): Promise<Counters> {
  let previous = await counters(page, cdp);
  for (let attempt = 0; attempt < 40; attempt++) {
    await page.evaluate(
      () =>
        new Promise<void>((resolve) =>
          requestAnimationFrame(() =>
            requestAnimationFrame(() =>
              typeof requestIdleCallback === 'function'
                ? requestIdleCallback(() => resolve(), { timeout: 1000 })
                : setTimeout(resolve, 50),
            ),
          ),
        ),
    );
    const current = await counters(page, cdp);
    if (current.listeners === previous.listeners && current.nodes === previous.nodes && current.callbacks === previous.callbacks) {
      return current;
    }
    previous = current;
  }
  return previous;
}

async function openAndClosePalette(page: Page) {
  await page.keyboard.press('Control+k');
  await expect(page.locator('.ff-cp')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.locator('.ff-cp')).toHaveCount(0);
}

async function openAndCloseSettings(page: Page) {
  await page.keyboard.press('Control+,');
  const items = page.locator('.ff-settings-sheet__nav-item');
  await expect(items.first()).toBeVisible();
  for (let i = 0; i < Math.min(3, await items.count()); i++) await items.nth(i).click();
  await page.keyboard.press('Escape');
  await expect(items.first()).toHaveCount(0);
}

async function expandAndCollapsePlayer(page: Page) {
  await page.locator('.ff-player-compact__expand-btn').click();
  await expect(page.locator('.ff-xp .waveform-display__zoom')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.locator('.ff-xp')).toHaveCount(0);
}

const title = CDP_COUNTERS
  ? 'opening and closing the palette, settings and the expanded player leaves no listeners, nodes or callbacks behind'
  : 'opening and closing the palette, settings and the expanded player leaves no callbacks behind (listener and node counters are Chromium-only)';

test(title, async ({ smoke }) => {
  test.setTimeout(180_000);
  const { page } = smoke;
  await page.evaluate(() => ((window as unknown as SmokeWindow).__smokeDetail = 'serve'));
  await loadByDoubleClick(page, 'Smoke Track 03');
  const cdp = CDP_COUNTERS ? await page.context().newCDPSession(page) : null;
  await cdp?.send('Memory.getDOMCounters');

  const cycle = async () => {
    await openAndClosePalette(page);
    await openAndCloseSettings(page);
    await expandAndCollapsePlayer(page);
  };
  await cycle();
  const before = await settledCounters(page, cdp);
  for (let i = 0; i < CYCLES; i++) await cycle();
  const after = await settledCounters(page, cdp);

  const detail = `before ${JSON.stringify(before)} after ${JSON.stringify(after)}`;
  expect(after.callbacks - before.callbacks, `registered event callbacks, ${detail}`).toBe(0);
  if (!cdp) return;
  expect(after.listeners - before.listeners, `event listeners after ${CYCLES} cycles, ${detail}`).toBeLessThanOrEqual(2);
  expect(after.nodes - before.nodes, `DOM nodes, attached or detached, ${detail}`).toBeLessThanOrEqual(40);
});
