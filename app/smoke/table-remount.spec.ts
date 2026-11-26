import { test, expect, type Page } from '@playwright/test';
import { fixtureAnalysis, largeFixtureState } from './fixture.ts';
import { installTauriMock } from './tauri-mock.ts';

const markScroller = (page: Page) =>
  page.evaluate(() => {
    (document.querySelector('.ff-table__scroll') as HTMLElement).dataset.smokeMark = '';
  });
const scrollerMarked = (page: Page) => page.evaluate(() => (document.querySelector('.ff-table__scroll') as HTMLElement).dataset.smokeMark !== undefined);

test('the track scroller is rebuilt only when the list switches between tall and short, and focus comes back to the list', async ({ page }) => {
  await page.exposeFunction('__smokeAnalysis', (trackId: number) => fixtureAnalysis(trackId));
  await page.addInitScript(installTauriMock, largeFixtureState(1000));
  await page.goto('/');
  await page.locator('[data-track-row]').first().waitFor();
  const list = page.getByRole('listbox', { name: 'Tracks' });
  const scroller = page.locator('.ff-table__scroll');

  await list.focus();
  await scroller.evaluate((el) => (el.scrollTop = 5000));
  await markScroller(page);
  await page.locator('.ff-colh__cell[data-col-key="title"] .ff-colh__label').click();
  expect(await scrollerMarked(page)).toBe(true);

  await list.focus();
  await page.keyboard.press('Control+k');
  await expect(page.locator('.ff-cp')).toBeVisible();
  await page.keyboard.type('zzqxjv');
  await expect(page.locator('[data-track-row]')).toHaveCount(0);
  expect(await scrollerMarked(page)).toBe(false);

  await markScroller(page);
  await page.keyboard.press('Control+a');
  await page.keyboard.press('Backspace');
  await page.keyboard.press('Escape');
  await expect(page.locator('.ff-cp')).toHaveCount(0);
  await expect(page.locator('[data-track-row]').first()).toBeVisible();
  expect(await scrollerMarked(page)).toBe(false);
  expect(await scroller.evaluate((el) => el.scrollTop)).toBeLessThan(1000);
  await expect(list).toBeFocused();
});
