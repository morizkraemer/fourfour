import { test, expect, type Page } from '@playwright/test';
import { fixtureAnalysis, fixtureState } from './fixture.ts';
import { installTauriMock, type SmokeWindow } from './tauri-mock.ts';

async function openApp(page: Page, state: ReturnType<typeof fixtureState>) {
  await page.exposeFunction('__smokeAnalysis', (trackId: number) => fixtureAnalysis(trackId));
  await page.addInitScript(installTauriMock, state);
  await page.addInitScript(() => ((window as unknown as SmokeWindow).__smokeHoldLoad = true));
  await page.goto('/');
}

const releaseLoad = (page: Page) => page.evaluate(() => (window as unknown as SmokeWindow).__smokeReleaseLoad?.());

test('skeleton rows show instead of the empty state until the library loads', async ({ page }) => {
  await openApp(page, fixtureState());
  await expect(page.locator('[data-table-skeleton]')).toBeVisible();
  await expect(page.getByText('No tracks loaded')).toHaveCount(0);
  await releaseLoad(page);
  await page.locator('[data-track-row]').first().waitFor();
  await expect(page.locator('[data-table-skeleton]')).toHaveCount(0);
});

test('a library that really has no tracks shows the empty state after loading', async ({ page }) => {
  await openApp(page, { ...fixtureState(), tracks: [], playlists: [] });
  await expect(page.getByText('No tracks loaded')).toHaveCount(0);
  await releaseLoad(page);
  await expect(page.getByText('No tracks loaded')).toBeVisible();
  await expect(page.locator('[data-table-skeleton]')).toHaveCount(0);
});
