import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import type { Page } from '@playwright/test';
import { TRACK_COUNT } from './fixture.ts';
import { expect, rows, rowByTitle, test } from './app.ts';

const SHOTS = resolve(import.meta.dirname, '../../.agent/files');
mkdirSync(SHOTS, { recursive: true });

const statusButton = (page: Page) => page.locator('[data-library-status]');
const menuItems = (page: Page) => page.locator('.ff-context-menu .ff-menu__item');
const healthDialog = (page: Page) => page.locator('[data-health-dialog]');
const row = (page: Page, n: number) => rowByTitle(page, `Smoke Track ${String(n).padStart(2, '0')}`);

async function withProblems(page: Page, missing: number[], unreadable: number[] = [], notFound: number[] = []) {
  await page.addInitScript(
    ([m, u, nf]) => {
      const w = window as unknown as Record<string, unknown>;
      w.__smokeMissing = m;
      w.__smokeUnreadable = u;
      w.__smokeNotFound = nf;
    },
    [missing, unreadable, notFound] as const,
  );
  await page.reload();
  await rows(page).first().waitFor();
}

test('no status button and a calm empty status column while every file is found', async ({ smoke: { page } }) => {
  await expect(page.locator('.ff-colh [data-col-key="status"]')).toHaveCount(1);
  await expect(page.locator('[data-track-row][data-file-status]')).toHaveCount(0);
  await expect(statusButton(page)).toHaveCount(0);
});

test('missing and unreadable rows get a status mark, a tooltip and do not play', async ({ smoke }) => {
  const { page } = smoke;
  await withProblems(page, [3, 5], [7]);

  await expect(statusButton(page)).toHaveText('2 missing · 1 unreadable');
  await expect(row(page, 3)).toHaveAttribute('data-file-status', 'missing');
  await expect(row(page, 3).locator('.ff-trow__cell--status')).toHaveText('!');
  await expect(row(page, 7)).toHaveAttribute('data-file-status', 'error');
  await expect(row(page, 7).locator('.ff-trow__cell--status')).toHaveText('×');
  await expect(row(page, 3)).toHaveAttribute('title', /File missing: \/Music\/smoke\/Smoke Track 03\.mp3/);
  await expect(row(page, 1)).not.toHaveAttribute('data-file-status');

  await row(page, 3).dblclick();
  await page.waitForTimeout(300);
  expect(await smoke.invokes('playback_play')).toEqual([]);
  await page.screenshot({ path: `${SHOTS}/status-column.png`, clip: { x: 0, y: 0, width: 1600, height: 420 } });
});

test('the status column hides and shows from the column menu', async ({ smoke: { page } }) => {
  await page.locator('.ff-colh [data-col-key="title"]').click({ button: 'right' });
  await menuItems(page).filter({ hasText: 'File status' }).click();
  await expect(page.locator('.ff-colh [data-col-key="status"]')).toHaveCount(0);
  await menuItems(page).filter({ hasText: 'File status' }).click();
  await expect(page.locator('.ff-colh [data-col-key="status"]')).toHaveCount(1);
});

test('the status button popover lists one action per line and filters to missing tracks', async ({ smoke }) => {
  const { page } = smoke;
  await withProblems(page, [3, 5]);

  await statusButton(page).click();
  await expect(menuItems(page)).toHaveText(['Relocate missing…', 'Show missing tracks', 'Check again', 'Library check results…']);
  await page.screenshot({ path: `${SHOTS}/topbar-popover.png`, clip: { x: 800, y: 0, width: 800, height: 260 } });

  await menuItems(page).filter({ hasText: 'Show missing tracks' }).click();
  await expect(rows(page)).toHaveCount(2);
  await expect(page.locator('.ff-gheader__title')).toHaveText('Missing tracks');

  await statusButton(page).click();
  await menuItems(page).filter({ hasText: 'Show all tracks' }).click();
  await expect(rows(page)).toHaveCount(TRACK_COUNT);

  const before = (await smoke.invokes('check_library_files')).length;
  await statusButton(page).click();
  await menuItems(page).filter({ hasText: 'Check again' }).click();
  await expect.poll(async () => (await smoke.invokes('check_library_files')).length).toBe(before + 1);
});

test('relocate relinks matches, reports the rest and offers Locate…', async ({ smoke }) => {
  const { page } = smoke;
  await withProblems(page, [3, 5], [], [5]);

  await statusButton(page).click();
  await menuItems(page).filter({ hasText: 'Relocate missing…' }).click();

  await expect(healthDialog(page)).toBeVisible();
  expect(await smoke.invokes('relocate_missing_in_folder')).toEqual([{ folder: '/Music/moved' }]);
  await expect(page.locator('[data-relocate-report]')).toContainText('1 track relinked, 1 not found.');
  await expect(row(page, 3)).toHaveAttribute('data-file-status', 'relocated');
  await expect(row(page, 3).locator('.ff-trow__cell--status')).toHaveText('✓');
  await expect(statusButton(page)).toHaveText('1 missing');
  await page.screenshot({ path: `${SHOTS}/relocate-dialog.png` });

  await page.locator('[data-relocate-unresolved="5"]').getByRole('button', { name: 'Locate…' }).click();
  await expect.poll(() => smoke.invokes('locate_track_file')).toEqual([{ trackId: 5, path: '/Music/moved' }]);
  await expect(statusButton(page)).toHaveCount(0);
  await expect(page.locator('[data-relocate-unresolved]')).toHaveCount(0);

  await page.locator('.ff-dialog').getByRole('button', { name: 'Done' }).click();
  await expect(healthDialog(page)).toHaveCount(0);
});

test('the results dialog opens from the popover', async ({ smoke: { page } }) => {
  await withProblems(page, [3], [7]);
  await statusButton(page).click();
  await menuItems(page).filter({ hasText: 'Library check results…' }).click();
  await expect(healthDialog(page)).toBeVisible();
  await expect(page.locator('[data-health-line="ok"]')).toHaveText(`${TRACK_COUNT - 2} tracks are fine`);
  await expect(page.locator('[data-health-line="missing"]')).toContainText('1 file is missing');
  await expect(page.locator('[data-health-line="error"]')).toContainText("1 file can't be read");
  await page.screenshot({ path: `${SHOTS}/health-dialog.png` });
});

test('Settings → Library shows the last check and checks again', async ({ smoke }) => {
  const { page } = smoke;
  await withProblems(page, [3]);
  await page.keyboard.press('Control+,');
  const card = page.locator('[data-settings-file-check]');
  await expect(card).toContainText('Last check today');
  await expect(card).toContainText('1 missing');
  const before = (await smoke.invokes('check_library_files')).length;
  await card.getByRole('button', { name: 'Check now' }).click();
  await expect.poll(async () => (await smoke.invokes('check_library_files')).length).toBe(before + 1);
  await page.screenshot({ path: `${SHOTS}/settings-library.png` });
});
