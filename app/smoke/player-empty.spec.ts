import type { Page } from '@playwright/test';
import { expect, loadByDoubleClick, test } from './app.ts';
import type { SmokeWindow } from './tauri-mock.ts';

async function expand(page: Page) {
  await page.locator('.ff-player-compact__expand-btn').click();
  await expect(page.locator('.ff-xp')).toBeVisible();
}

function gridPixelsAtTop(page: Page, selector: string) {
  return page.evaluate((sel) => {
    const canvas = document.querySelector(sel) as HTMLCanvasElement;
    const copy = document.createElement('canvas');
    copy.width = canvas.width;
    copy.height = 2;
    const reader = copy.getContext('2d', { willReadFrequently: true })!;
    reader.drawImage(canvas, 0, 0);
    const row = reader.getImageData(0, 1, canvas.width, 1).data;
    const sums = Array.from({ length: canvas.width }, (_, x) => row[x * 4] + row[x * 4 + 1] + row[x * 4 + 2]);
    const base = Math.min(...sums);
    return sums.filter((sum) => sum > base + 24).length;
  }, selector);
}

test('the overview above the big waveform draws the beat grid', async ({ smoke: { page } }) => {
  await loadByDoubleClick(page, 'Smoke Track 03');
  await expand(page);
  await expect.poll(() => gridPixelsAtTop(page, '.ff-xp .waveform-display__overview')).toBeGreaterThan(3);
});

test('a track without analysis shows a calm labelled empty state in the big waveform', async ({ smoke: { page } }) => {
  await page.evaluate(() => ((window as unknown as SmokeWindow).__smokeNoAnalysis = [5]));
  await loadByDoubleClick(page, 'Smoke Track 05');
  await expand(page);
  await expect(page.locator('.ff-xp .ff-waveform-view__empty-label')).toHaveText(/^(Analyzing|Not analysed yet)$/);
});

test('the big waveform empty state is not shown for an analysed track', async ({ smoke: { page } }) => {
  await loadByDoubleClick(page, 'Smoke Track 03');
  await expand(page);
  await expect(page.locator('.ff-xp .waveform-display__overview')).toBeVisible();
  await expect(page.locator('.ff-xp .ff-waveform-view__empty')).toHaveCount(0);
});
