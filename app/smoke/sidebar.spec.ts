import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import type { Page } from '@playwright/test';
import { expect, rows, test } from './app.ts';
import type { SmokeWindow } from './tauri-mock.ts';

const SHOTS = resolve(import.meta.dirname, '../../.agent/sidebar');
mkdirSync(SHOTS, { recursive: true });

const PEAK_TIME = JSON.stringify({ kind: 'playlist', id: 'Peak Time', label: 'Peak Time', playlistId: 1 });

async function open(page: Page, width = 420, withList = true) {
  await page.evaluate(
    ([w, list]) => {
      localStorage.setItem('fourfour.organize.expanded', 'true');
      localStorage.setItem('fourfour.organize.width', String(w));
      if (list) localStorage.setItem('fourfour.organize.target', list);
      else localStorage.removeItem('fourfour.organize.target');
    },
    [width, withList ? PEAK_TIME : ''] as const,
  );
  await page.reload();
  await rows(page).first().waitFor();
  await page.locator('.ff-organize--expanded').waitFor();
}

const box = async (page: Page, selector: string) => (await page.locator(selector).first().boundingBox())!;

async function expectHeaderFullWidth(page: Page) {
  const header = await box(page, '.ff-gheader');
  const sidebar = await box(page, '.ff-sidebar');
  const viewport = page.viewportSize()!;
  expect(header.x).toBeCloseTo(sidebar.x + sidebar.width, 0);
  expect(header.x + header.width).toBeCloseTo(viewport.width, 0);
}

test('closed sidebar: header spans to the window edge', async ({ smoke: { page } }) => {
  await expectHeaderFullWidth(page);
  await page.screenshot({ path: `${SHOTS}/closed.png` });
});

test('open sidebar sits below a full-width top bar at default width', async ({ smoke: { page } }) => {
  const closedHeader = await box(page, '.ff-gheader');
  await open(page);
  await expectHeaderFullWidth(page);
  const header = await box(page, '.ff-gheader');
  const organize = await box(page, '.ff-organize--expanded');
  expect(organize.y).toBeGreaterThanOrEqual(header.y + header.height);
  expect(header.width).toBeCloseTo(closedHeader.width, 0);
  await page.screenshot({ path: `${SHOTS}/open-default.png` });
});

test('open sidebar at max width leaves the top bar full width', async ({ smoke: { page } }) => {
  await open(page);
  const handle = await box(page, '.ff-organize__resize-handle');
  await page.mouse.move(handle.x + 2, handle.y + 100);
  await page.mouse.down();
  await page.mouse.move(2, handle.y + 100, { steps: 8 });
  await page.mouse.up();
  const sidebar = await box(page, '.ff-sidebar');
  const organize = await box(page, '.ff-organize--expanded');
  expect(organize.width).toBeGreaterThan(page.viewportSize()!.width - sidebar.width - 4);
  await expectHeaderFullWidth(page);
  await page.screenshot({ path: `${SHOTS}/open-max.png` });
});

test('resize handle still changes the sidebar width', async ({ smoke: { page } }) => {
  await open(page);
  const before = await box(page, '.ff-organize--expanded');
  const handle = await box(page, '.ff-organize__resize-handle');
  await page.mouse.move(handle.x + 2, handle.y + 100);
  await page.mouse.down();
  await page.mouse.move(handle.x - 98, handle.y + 100, { steps: 5 });
  await page.mouse.up();
  const after = await box(page, '.ff-organize--expanded');
  expect(after.width).toBeGreaterThan(before.width + 80);
});

test('empty sidebar is capped to the left sidebar width, a loaded list expands it', async ({ smoke: { page } }) => {
  await open(page, 420, false);
  const sidebar = await box(page, '.ff-sidebar');
  const empty = await box(page, '.ff-organize--expanded');
  expect(empty.width).toBeCloseTo(sidebar.width, 0);
  await expect(page.locator('.ff-organize__resize-handle')).toHaveCount(0);
  await open(page, 420, true);
  expect((await box(page, '.ff-organize--expanded')).width).toBeCloseTo(420, 0);
});

test('empty sidebar fills a user width narrower than the cap, a loaded list keeps the user width', async ({ smoke: { page } }) => {
  await open(page, 210, false);
  expect((await box(page, '.ff-organize--expanded')).width).toBeCloseTo(210, 0);
  await open(page, 480, true);
  expect((await box(page, '.ff-organize--expanded')).width).toBeCloseTo(480, 0);
});

test('Analyzing chip stays visible and clickable with the sidebar open', async ({ smoke: { page } }) => {
  await open(page);
  await page.evaluate(() => (window as unknown as SmokeWindow).__smokeEmit('analysis-progress', { current: 1, total: 3, message: 'Analyzing' }));
  const chip = page.locator('[data-library-status]');
  await expect(chip).toBeVisible();
  await expect(chip).toContainText('1 / 3');
  await chip.click();
  await expect(page.locator('[data-library-activity-menu]')).toBeVisible();
  await page.screenshot({ path: `${SHOTS}/open-analyzing.png` });
});

test('top bar keeps its drag region while the sidebar is open', async ({ smoke: { page } }) => {
  await open(page);
  await expect(page.locator('.ff-gheader[data-tauri-drag-region]')).toHaveCount(1);
  await expect(page.locator('.ff-gheader__ctx[data-tauri-drag-region]')).toHaveCount(1);
  await expect(page.locator('.ff-gheader__right[data-tauri-drag-region]')).toHaveCount(1);
});
