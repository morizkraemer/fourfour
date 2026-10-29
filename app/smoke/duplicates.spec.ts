import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import type { Page } from '@playwright/test';
import { expect, test } from './app.ts';

const SHOTS = resolve(import.meta.dirname, '../../.agent/dupes');
mkdirSync(SHOTS, { recursive: true });

const NAMES = [
  ['Glass Harbor (Original Mix).aiff', 'Glass Harbor', 'Ode & Ash', 425],
  ['ode_ash-glass_harbor-320.mp3', 'Glass Harbor', 'Ode & Ash', 425],
  ['Silent Hours (Extended).wav', 'Silent Hours', 'Mira Sol', 402],
  ['Copper Lines.flac', 'Copper Lines', 'Hale', 380],
  ['Afterimage - Mira Sol.mp3', 'Afterimage', 'Mira Sol', 491],
  ['Low Tide Ritual (Club).aiff', 'Low Tide Ritual', 'Tomas Reyne', 358],
  ['Paper Moons.wav', 'Paper Moons', 'Ivo Brandt', 363],
  ['Harbor Light (Promo).mp3', 'Harbor Light', 'Ode & Ash', 454],
  ['Slow Orbit.flac', 'Slow Orbit', 'Tomas Reyne', 347],
  ['Night Signal.aiff', 'Night Signal', 'Hale', 411],
  ['Warm Static (Original).wav', 'Warm Static', 'Mira Sol', 432],
  ['Tin Rooftops.mp3', 'Tin Rooftops', 'Ivo Brandt', 339],
] as const;

function duplicates(count: number) {
  return NAMES.slice(0, count).map(([file, title, artist, secs], i) => ({
    new_path: `/Downloads/${file}`,
    file_name: file,
    existing_id: i + 1,
    title,
    artist,
    duration_secs: secs,
    reason: 'same_tags',
  }));
}

async function importFolderWithDuplicates(page: Page, count: number) {
  await page.evaluate((rows) => ((window as unknown as { __smokeDuplicates: unknown[] }).__smokeDuplicates = rows), duplicates(count));
  await page.keyboard.press('Control+k');
  await page.keyboard.type('>import folder');
  await page.keyboard.press('Enter');
}

const dialog = (page: Page) => page.locator('.ff-dialog').filter({ has: page.locator('[data-duplicates-dialog]') });

test('Skip duplicates is the default and commits without them', async ({ smoke }) => {
  const { page } = smoke;
  await importFolderWithDuplicates(page, 12);

  await expect(dialog(page).locator('.ff-dialog__title')).toHaveText('12 tracks look like ones you already have');
  await expect(page.locator('[data-duplicate-row]')).toHaveCount(12);
  await expect(page.locator('[data-duplicate-row]').first()).toContainText('7:05');
  await expect(dialog(page).getByRole('button', { name: 'Skip duplicates' })).toBeFocused();
  const scrollbar = await page.locator('.ff-dupes__scroll').evaluate((el) => getComputedStyle(el).scrollbarWidth);
  expect(scrollbar).toBe('none');
  await page.screenshot({ path: `${SHOTS}/duplicates-dialog.png` });

  await page.keyboard.press('Enter');
  await expect(dialog(page)).toHaveCount(0);
  expect(await smoke.invokes('plan_import')).toEqual([{ paths: ['/Music/moved'] }]);
  expect(await smoke.invokes('commit_import')).toEqual([{ token: 1, skipDuplicates: true }]);
});

test('Import anyway commits with the duplicates', async ({ smoke }) => {
  const { page } = smoke;
  await importFolderWithDuplicates(page, 1);

  await expect(dialog(page).locator('.ff-dialog__title')).toHaveText('1 track looks like one you already have');
  await page.screenshot({ path: `${SHOTS}/duplicates-dialog-one.png` });
  await dialog(page).getByRole('button', { name: 'Import anyway' }).click();
  await expect(dialog(page)).toHaveCount(0);
  expect(await smoke.invokes('commit_import')).toEqual([{ token: 1, skipDuplicates: false }]);
});

test('no dialog when nothing looks like a duplicate', async ({ smoke }) => {
  const { page } = smoke;
  await importFolderWithDuplicates(page, 0);
  await expect.poll(() => smoke.invokes('commit_import')).toEqual([{ token: 1, skipDuplicates: true }]);
  await expect(page.locator('[data-duplicates-dialog]')).toHaveCount(0);
});

test('the list grows to 60% of the window before it scrolls, keeping the fade', async ({ smoke }) => {
  const { page } = smoke;
  const many = Array.from({ length: 40 }, (_, i) => ({ ...duplicates(1)[0], new_path: `/Downloads/copy-${i}.mp3`, file_name: `copy-${i}.mp3`, existing_id: i + 1 }));
  await page.evaluate((rows) => ((window as unknown as { __smokeDuplicates: unknown[] }).__smokeDuplicates = rows), many);
  await page.keyboard.press('Control+k');
  await page.keyboard.type('>import folder');
  await page.keyboard.press('Enter');
  const scroll = page.locator('.ff-dupes__scroll');
  await expect(scroll).toBeVisible();
  const { client, scrollHeight, inner, mask } = await scroll.evaluate((el) => ({
    client: el.clientHeight,
    scrollHeight: el.scrollHeight,
    inner: window.innerHeight,
    mask: getComputedStyle(el).maskImage || getComputedStyle(el).webkitMaskImage,
  }));
  expect(client).toBeGreaterThan(inner * 0.55);
  expect(client).toBeLessThanOrEqual(inner * 0.6 + 1);
  expect(scrollHeight).toBeGreaterThan(client);
  expect(mask).not.toBe('none');
  await page.screenshot({ path: `${SHOTS}/duplicates-dialog-tall.png` });
  await page.keyboard.press('Enter');

  await importFolderWithDuplicates(page, 3);
  const short = await page.locator('.ff-dupes__scroll').evaluate((el) => getComputedStyle(el).maskImage);
  expect(short).toBe('none');
});
