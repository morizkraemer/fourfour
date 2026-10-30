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
  await setup(page, 900, false);
  const icon = page.locator('.ff-gheader [data-search-icon]');
  await expect(icon).toBeVisible();
  await expect(page.locator('.ff-sidebar [data-search-icon]')).toHaveCount(0);
  await expect(page.locator('.ff-gheader__search input')).toBeHidden();
  await page.screenshot({ path: `${SHOTS}/icon-state.png` });
});

test('very narrow centre column moves the search icon into the left sidebar', async ({ smoke: { page } }) => {
  await setup(page, 670, false);
  const icon = page.locator('.ff-sidebar [data-search-icon]');
  await expect(icon).toBeVisible();
  await expect(page.locator('.ff-gheader [data-search-icon]')).toHaveCount(0);
  const sidebar = await box(page, '.ff-sidebar');
  const iconBox = (await icon.boundingBox())!;
  expect(iconBox.x + iconBox.width).toBeLessThanOrEqual(sidebar.x + sidebar.width);
  await page.screenshot({ path: `${SHOTS}/moved-to-sidebar-state.png` });
});

const COLLAPSED = [['top-bar-icon', 900], ['sidebar-icon', 670]] as const;
const modalInput = (page: Page) => page.locator('.ff-cp-overlay input');
const modalBox = (page: Page) => box(page, '.ff-cp-overlay .ff-cp');

async function expectCentredModal(page: Page, width: number) {
  const dialog = await modalBox(page);
  expect(Math.abs(dialog.x + dialog.width / 2 - width / 2)).toBeLessThanOrEqual(2);
  expect(dialog.width).toBeGreaterThanOrEqual(Math.min(600, width - 32));
  await expect(page.locator('.ff-gheader__search input')).toBeHidden();
  const truncated = await page.locator('.ff-cp-overlay .ff-cp__row-name').evaluateAll((els) =>
    els.filter((el) => el.scrollWidth > el.clientWidth).map((el) => el.textContent),
  );
  expect(truncated).toEqual([]);
  await expect(page.locator('.ff-cp-overlay .ff-cp__section-head', { hasText: 'Go to' })).toBeVisible();
}

for (const [label, width] of COLLAPSED) {
  for (const trigger of ['icon', 'shortcut'] as const) {
    test(`${trigger} opens a centred search modal from the ${label} state`, async ({ smoke: { page } }) => {
      await setup(page, width, false);
      if (trigger === 'icon') await page.locator('[data-search-icon]').click();
      else await page.keyboard.press('ControlOrMeta+f');
      await expect(modalInput(page)).toBeFocused();
      await expectCentredModal(page, width);
      await page.screenshot({ path: `${SHOTS}/modal-${label}-${trigger}.png` });
    });
  }

  test(`the ${label} search modal filters the table, Esc and backdrop close it`, async ({ smoke: { page } }) => {
    await setup(page, width, false);
    await page.locator('[data-search-icon]').click();
    await modalInput(page).fill('Track 07');
    await expect(rows(page)).not.toHaveCount(TRACK_COUNT);
    await page.screenshot({ path: `${SHOTS}/modal-${label}-filtered.png` });
    await page.keyboard.press('Escape');
    await expect(modalInput(page)).toBeHidden();
    await page.locator('[data-search-icon]').click();
    await expect(modalInput(page)).toBeVisible();
    await page.mouse.click(4, 4);
    await expect(modalInput(page)).toBeHidden();
  });
}

test('the full field state keeps the dropdown under the top bar field', async ({ smoke: { page } }) => {
  await setup(page, 1600, false);
  await page.locator('.ff-gheader__search input').click();
  await expect(page.locator('.ff-cp--anchored')).toBeVisible();
  await expect(page.locator('.ff-cp-overlay')).toHaveCount(0);
  await page.screenshot({ path: `${SHOTS}/field-dropdown.png` });
});

test('the track count is the status chip button and opens the activity panel', async ({ smoke: { page } }) => {
  const chip = page.locator('[data-library-status]');
  await expect(chip).toContainText('tracks');
  expect(await chip.evaluate((el) => el.tagName)).toBe('BUTTON');
  expect(await chip.evaluate((el) => getComputedStyle(el).cursor)).toBe('pointer');
  await chip.click();
  await expect(page.locator('[data-library-activity-menu]')).toBeVisible();
});

const centreY = (b: { y: number; height: number }) => b.y + b.height / 2;

test("top-bar icon shares the title's vertical centre", async ({ smoke: { page } }) => {
  await setup(page, 900, false);
  const icon = page.locator('.ff-gheader [data-search-icon]');
  await expect(icon).toBeVisible();
  const iconBox = (await icon.boundingBox())!;
  const title = await box(page, '.ff-gheader__title');
  expect(Math.abs(centreY(iconBox) - centreY(title))).toBeLessThanOrEqual(1);
  const right = page.viewportSize()!.width;
  expect(right - (iconBox.x + iconBox.width)).toBeLessThanOrEqual(16);
});

test("sidebar icon shares the title's vertical centre", async ({ smoke: { page } }) => {
  await setup(page, 670, false);
  const icon = page.locator('.ff-sidebar [data-search-icon]');
  await expect(icon).toBeVisible();
  const iconBox = (await icon.boundingBox())!;
  const title = await box(page, '.ff-gheader__title');
  expect(Math.abs(centreY(iconBox) - centreY(title))).toBeLessThanOrEqual(1);
  const sidebar = await box(page, '.ff-sidebar');
  expect(iconBox.x + iconBox.width).toBeLessThanOrEqual(sidebar.x + sidebar.width);
});
