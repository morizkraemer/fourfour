import type { Page } from '@playwright/test';
import { expect, loadByDoubleClick, test } from './app.ts';
import type { SmokeWindow } from './tauri-mock.ts';

const TRACK = 'Smoke Track 03';

type Recorded = { sources: string[]; first: string | null };

async function expandPaused(page: Page) {
  await loadByDoubleClick(page, TRACK);
  await page.locator('.ff-player-compact__expand-btn').click();
  await expect(page.locator('.ff-xp .waveform-display__zoom')).toBeVisible();
  if ((await page.locator('.ff-xp__play').getAttribute('aria-label')) === 'Pause') await page.keyboard.press('Space');
  await expect(page.locator('.ff-xp__play')).toHaveAttribute('aria-label', 'Play');
}

function zoomPixels(page: Page) {
  return page.evaluate(() => {
    const canvas = document.querySelector('.ff-xp .waveform-display__zoom') as HTMLCanvasElement;
    return canvas.toDataURL();
  });
}

function recordZoomSources(page: Page) {
  return page.evaluate(() => {
    const canvas = document.querySelector('.ff-xp .waveform-display__zoom') as HTMLCanvasElement;
    const recorded: Recorded = { sources: [canvas.dataset.bands ?? ''], first: null };
    (window as unknown as { __zoomSources: Recorded }).__zoomSources = recorded;
    new MutationObserver(() => {
      recorded.sources.push(canvas.dataset.bands ?? '');
      if (canvas.dataset.bands !== 'pending' && !recorded.first) {
        recorded.first = canvas.toDataURL();
      }
    }).observe(canvas, { attributes: true, attributeFilter: ['data-bands'] });
  });
}

test('the zoom view waits for the high-resolution series and first draws it exactly as it stays', async ({ smoke: { page } }) => {
  await page.evaluate(() => ((window as unknown as SmokeWindow).__smokeDetail = 'hold'));
  await expandPaused(page);
  await expect(page.locator('.ff-xp .waveform-display__zoom')).toHaveAttribute('data-bands', 'pending');
  await recordZoomSources(page);

  await page.evaluate(() => (window as unknown as SmokeWindow).__smokeReleaseDetail?.());
  await expect(page.locator('.ff-xp .waveform-display__zoom')).toHaveAttribute('data-bands', 'detail');
  await page.waitForTimeout(400);
  const settled = await zoomPixels(page);
  const recorded = await page.evaluate(() => (window as unknown as { __zoomSources: Recorded }).__zoomSources);

  expect(new Set(recorded.sources)).toEqual(new Set(['pending', 'detail']));
  expect(recorded.first).not.toBeNull();
  expect(recorded.first === settled, 'the first drawn zoom view is pixel for pixel the settled one').toBe(true);
});

test('without a high-resolution series the zoom view draws the analysis columns', async ({ smoke: { page } }) => {
  await expandPaused(page);
  await expect(page.locator('.ff-xp .waveform-display__zoom')).toHaveAttribute('data-bands', 'columns');
});
