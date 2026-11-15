import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { expect, test } from './app.ts';

const SHOTS = resolve(import.meta.dirname, '../../.agent/about');
mkdirSync(SHOTS, { recursive: true });

test('settings has an about page with the icon, version and credits', async ({ smoke }) => {
  const page = smoke.page;
  await page.keyboard.press('Control+,');
  await page.locator('.ff-settings-sheet__nav-item').filter({ hasText: 'About' }).click();
  await expect(page.locator('.ff-settings-sheet__heading')).toHaveText('About');

  const about = page.locator('[data-settings-about]');
  const icon = about.locator('img');
  await expect(icon).toBeVisible();
  expect(await icon.evaluate((img: HTMLImageElement) => img.complete && img.naturalWidth > 0)).toBe(true);
  await expect(page.locator('[data-about-version]')).toHaveText(/^Version \S+/);
  await expect(page.locator('[data-about-credits]')).toContainText('Beat This!');
  await page.screenshot({ path: `${SHOTS}/settings-about.png` });
});
