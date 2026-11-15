import type { Page } from '@playwright/test';
import { expect, rowByTitle, rows, test } from './app.ts';

const inspector = (page: Page) => page.locator('.ff-inspector');
const autoOpenSwitch = (page: Page) => page.getByRole('switch', { name: 'Open inspector when selecting a track' });

async function closeInspector(page: Page) {
  await rowByTitle(page, 'Smoke Track 01').click();
  await page.getByRole('button', { name: 'Close inspector' }).click();
  await expect(inspector(page)).toHaveCount(0);
}

async function openLibrarySettings(page: Page) {
  await page.keyboard.press('Control+,');
  await page.locator('.ff-settings-sheet__nav-item').filter({ hasText: 'Library' }).click();
}

test('selecting a track reopens a closed inspector', async ({ smoke: { page } }) => {
  await closeInspector(page);
  await rowByTitle(page, 'Smoke Track 02').click();
  await expect(inspector(page)).toBeVisible();
});

test('arrow key selection reopens a closed inspector', async ({ smoke: { page } }) => {
  await closeInspector(page);
  const list = page.getByRole('listbox', { name: 'Tracks' });
  for (let i = 0; i < 6 && !(await list.evaluate((el) => el === document.activeElement)); i++) {
    await page.keyboard.press('F6');
  }
  await expect(list).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await expect(rows(page).nth(1)).toHaveAttribute('aria-selected', 'true');
  await expect(inspector(page)).toBeVisible();
});

test('with the setting off the inspector stays closed, and the setting survives a reload', async ({ smoke: { page } }) => {
  await closeInspector(page);
  await openLibrarySettings(page);
  await expect(autoOpenSwitch(page)).toHaveAttribute('aria-checked', 'true');
  await autoOpenSwitch(page).click();
  await expect(autoOpenSwitch(page)).toHaveAttribute('aria-checked', 'false');
  await page.keyboard.press('Escape');
  await expect(page.locator('.ff-settings-sheet__heading')).toHaveCount(0);

  await rowByTitle(page, 'Smoke Track 02').click();
  await expect(inspector(page)).toHaveCount(0);

  await page.reload();
  await rowByTitle(page, 'Smoke Track 03').click();
  await page.getByRole('button', { name: 'Close inspector' }).click();
  await rowByTitle(page, 'Smoke Track 02').click();
  await expect(inspector(page)).toHaveCount(0);
  await openLibrarySettings(page);
  await expect(autoOpenSwitch(page)).toHaveAttribute('aria-checked', 'false');
});
