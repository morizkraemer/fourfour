import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import type { Page } from '@playwright/test';
import { TRACK_COUNT } from './fixture.ts';
import { expect, rows, test } from './app.ts';

const SHOTS = resolve(import.meta.dirname, '../../.agent/topbar');
mkdirSync(SHOTS, { recursive: true });

async function setup(page: Page, width: number, organizeOpen: boolean, panelWidth = 420) {
  await page.setViewportSize({ width, height: 900 });
  await page.evaluate(
    ([open, panel]) => {
      localStorage.setItem('fourfour.organize.expanded', String(open));
      localStorage.setItem('fourfour.organize.width', String(panel));
    },
    [organizeOpen, panelWidth] as const,
  );
  await page.reload();
  await rows(page).first().waitFor();
}

const box = async (page: Page, selector: string) => (await page.locator(selector).first().boundingBox())!;
const fieldBox = (page: Page) => box(page, '.ff-gheader__search');
const headerBox = (page: Page) => box(page, '.ff-gheader');

test('search keeps its width when the right sidebar opens at a wide window', async ({ smoke: { page } }) => {
  await setup(page, 1600, false);
  const closed = await fieldBox(page);
  await setup(page, 1600, true);
  const open = await fieldBox(page);
  expect(open.width).toBeCloseTo(closed.width, 0);
  expect(open.width).toBe(560);
  await page.screenshot({ path: `${SHOTS}/field-1600-organize-open.png` });
});

for (const width of [1600, 1200, 900, 700]) {
  for (const organizeOpen of [false, true]) {
    test(`search is never clipped at ${width} px, right sidebar ${organizeOpen ? 'open' : 'closed'}`, async ({ smoke: { page } }) => {
      await setup(page, width, organizeOpen);
      const header = await headerBox(page);
      const collapsed = (await page.locator('[data-search-icon]').count()) > 0;
      if (!collapsed) {
        const field = await fieldBox(page);
        expect(field.x).toBeGreaterThanOrEqual(header.x);
        expect(field.x + field.width).toBeLessThanOrEqual(header.x + header.width);
        expect(field.width).toBeGreaterThanOrEqual(240);
        await expect(page.locator('.ff-gheader__search input')).toBeVisible();
      } else {
        await expect(page.locator('[data-search-icon]')).toHaveCount(1);
      }
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
      await page.screenshot({ path: `${SHOTS}/w${width}-organize-${organizeOpen ? 'open' : 'closed'}.png` });
    });
  }
}

test('narrow centre column shows the search icon in the top bar', async ({ smoke: { page } }) => {
  await setup(page, 1100, true);
  const icon = page.locator('.ff-gheader [data-search-icon]');
  await expect(icon).toBeVisible();
  await expect(page.locator('.ff-sidebar [data-search-icon]')).toHaveCount(0);
  await expect(page.locator('.ff-gheader__search input')).toBeHidden();
  await page.screenshot({ path: `${SHOTS}/icon-state.png` });
});

test('very narrow centre column moves the search icon into the left sidebar', async ({ smoke: { page } }) => {
  await setup(page, 900, true);
  const icon = page.locator('.ff-sidebar [data-search-icon]');
  await expect(icon).toBeVisible();
  await expect(page.locator('.ff-gheader [data-search-icon]')).toHaveCount(0);
  const sidebar = await box(page, '.ff-sidebar');
  const iconBox = (await icon.boundingBox())!;
  expect(iconBox.x + iconBox.width).toBeLessThanOrEqual(sidebar.x + sidebar.width);
  await page.screenshot({ path: `${SHOTS}/moved-to-sidebar-state.png` });
});

for (const [label, width] of [['top-bar-icon', 1100], ['sidebar-icon', 900]] as const) {
  test(`clicking the ${label} opens the search over the top bar and typing filters`, async ({ smoke: { page } }) => {
    await setup(page, width, true);
    await page.locator('[data-search-icon]').click();
    const input = page.locator('.ff-gheader__search input');
    await expect(input).toBeVisible();
    await expect(input).toBeFocused();
    await input.fill('Track 07');
    await expect(rows(page)).not.toHaveCount(TRACK_COUNT);
    await page.screenshot({ path: `${SHOTS}/open-from-${label}.png` });
    await input.fill('');
    await page.keyboard.press('Escape');
    await expect(input).toBeHidden();
  });
}

test('the track count is plain text, not a button', async ({ smoke: { page } }) => {
  const text = page.locator('.ff-gheader__plain');
  await expect(text).toContainText('tracks');
  await expect(page.locator('.ff-gheader__chip--idle')).toHaveCount(0);
  expect(await text.evaluate((el) => getComputedStyle(el).cursor)).not.toBe('pointer');
});

test('Cmd/Ctrl+F opens the search while collapsed', async ({ smoke: { page } }) => {
  await setup(page, 1100, true);
  await page.keyboard.press('ControlOrMeta+f');
  await expect(page.locator('.ff-gheader__search input')).toBeVisible();
  await expect(page.locator('.ff-gheader__search input')).toBeFocused();
});

const centreY = (b: { y: number; height: number }) => b.y + b.height / 2;

for (const panelWidth of [320, 420, 520]) {
  test(`top-bar icon hugs the organize panel's left edge at panel width ${panelWidth}`, async ({ smoke: { page } }) => {
    await setup(page, 760 + panelWidth, true, panelWidth);
    const icon = page.locator('.ff-gheader [data-search-icon]');
    await expect(icon).toBeVisible();
    const iconBox = (await icon.boundingBox())!;
    const panel = await box(page, '.ff-organize--expanded');
    const gap = panel.x - (iconBox.x + iconBox.width);
    expect(gap).toBeGreaterThanOrEqual(0);
    expect(gap).toBeLessThanOrEqual(16);
    const title = await box(page, '.ff-gheader__title');
    expect(Math.abs(centreY(iconBox) - centreY(title))).toBeLessThanOrEqual(1);
    await page.screenshot({ path: `${SHOTS}/icon-top-bar-panel-${panelWidth}.png` });
  });

  test(`sidebar icon shares the title's vertical centre at panel width ${panelWidth}`, async ({ smoke: { page } }) => {
    await setup(page, 560 + panelWidth, true, panelWidth);
    const icon = page.locator('.ff-sidebar [data-search-icon]');
    await expect(icon).toBeVisible();
    const iconBox = (await icon.boundingBox())!;
    const title = await box(page, '.ff-gheader__title');
    expect(Math.abs(centreY(iconBox) - centreY(title))).toBeLessThanOrEqual(1);
    const sidebar = await box(page, '.ff-sidebar');
    expect(iconBox.x + iconBox.width).toBeLessThanOrEqual(sidebar.x + sidebar.width);
    await page.screenshot({ path: `${SHOTS}/icon-sidebar-panel-${panelWidth}.png` });
  });
}
