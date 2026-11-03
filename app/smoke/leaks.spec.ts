import type { CDPSession, Page } from '@playwright/test';
import { expect, loadByDoubleClick, test } from './app.ts';
import { loadFactor } from './load-factor.ts';
import type { SmokeWindow } from './tauri-mock.ts';

const CYCLES = 6;
const WARMUP_CYCLES = 2;
const STABLE_READS = 3;
const LISTENER_TOLERANCE = 2;
const NODE_TOLERANCE = 40;
const RECHECKS = 4;
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

function sameCounters(a: Counters, b: Counters): boolean {
  return a.listeners === b.listeners && a.nodes === b.nodes && a.callbacks === b.callbacks;
}

async function settledCounters(page: Page, cdp: CDPSession | null): Promise<Counters> {
  const minimumWindowMs = 150 * loadFactor();
  let previous = await counters(page, cdp);
  let stableSince = Date.now();
  let stableReads = 1;
  for (let attempt = 0; attempt < 80; attempt++) {
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
    if (sameCounters(current, previous)) {
      stableReads++;
      if (stableReads >= STABLE_READS && Date.now() - stableSince >= minimumWindowMs) return current;
    } else {
      stableReads = 1;
      stableSince = Date.now();
    }
    previous = current;
  }
  return previous;
}

function growthViolations(before: Counters, after: Counters, cdp: CDPSession | null): string[] {
  const violations: string[] = [];
  if (after.callbacks - before.callbacks !== 0) violations.push('registered event callbacks');
  if (!cdp) return violations;
  if (after.listeners - before.listeners > LISTENER_TOLERANCE) violations.push('event listeners');
  if (after.nodes - before.nodes > NODE_TOLERANCE) violations.push('DOM nodes, attached or detached');
  return violations;
}

async function measureGrowth(page: Page, cdp: CDPSession | null, cycle: () => Promise<void>) {
  for (let i = 0; i < WARMUP_CYCLES; i++) await cycle();
  const before = await settledCounters(page, cdp);
  for (let i = 0; i < CYCLES; i++) await cycle();
  let after = await settledCounters(page, cdp);
  for (let recheck = 0; recheck < RECHECKS && growthViolations(before, after, cdp).length > 0; recheck++) {
    after = await settledCounters(page, cdp);
  }
  return { before, after, violations: growthViolations(before, after, cdp) };
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
  test.setTimeout(180_000 * loadFactor());
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
  const { before, after, violations } = await measureGrowth(page, cdp, cycle);
  expect(violations, `growth after ${CYCLES} cycles, before ${JSON.stringify(before)} after ${JSON.stringify(after)}`).toEqual([]);
});

test('the growth check fails for a deliberate per-cycle leak', async ({ smoke }) => {
  test.setTimeout(180_000 * loadFactor());
  const { page } = smoke;
  const cdp = CDP_COUNTERS ? await page.context().newCDPSession(page) : null;
  await cdp?.send('Memory.getDOMCounters');

  const leakyCycle = async () => {
    await page.evaluate(() => {
      const w = window as unknown as Record<string, unknown>;
      const leaked = document.createElement('div');
      for (let i = 0; i < 10; i++) leaked.appendChild(document.createElement('span'));
      const keep = ((w.__leakedNodes as unknown[]) ??= []);
      keep.push(leaked);
      window.addEventListener('resize', () => keep.length);
      w[`_${900_000 + keep.length}`] = () => undefined;
    });
  };
  const { before, after, violations } = await measureGrowth(page, cdp, leakyCycle);
  const detail = `before ${JSON.stringify(before)} after ${JSON.stringify(after)}`;
  expect(violations, detail).toContain('registered event callbacks');
  if (cdp) {
    expect(violations, detail).toContain('event listeners');
    expect(violations, detail).toContain('DOM nodes, attached or detached');
  }
});
