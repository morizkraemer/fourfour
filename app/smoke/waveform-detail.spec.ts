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

test('selecting a row warms its high-resolution series and the next two; loading fetches the series at once', async ({ smoke }) => {
  const { page } = smoke;
  await page.locator('[data-track-row]').filter({ hasText: TRACK }).first().click();
  await expect.poll(async () => (await smoke.invokes('prefetch_waveform_detail')).length).toBe(3);
  const warmed = (await smoke.invokes('prefetch_waveform_detail')).map((args) => args.trackId);
  expect(new Set(warmed).size).toBe(3);

  await loadByDoubleClick(page, TRACK);
  await expect.poll(async () => (await smoke.invokes('get_waveform_detail')).length).toBeGreaterThan(0);
  expect((await smoke.invokes('get_waveform_detail'))[0].trackId).toBe(warmed[0]);
});

test('pressing a row warms its high-resolution series before the click lands', async ({ smoke }) => {
  const { page } = smoke;
  await page.locator('[data-track-row]').filter({ hasText: TRACK }).first().hover();
  await page.mouse.down();
  const pressed = await smoke.invokes('prefetch_waveform_detail');
  const warmed = pressed.map((args) => args.trackId);
  await page.mouse.up();
  expect(warmed).toHaveLength(1);
  expect(pressed[0].urgent, 'a pressed row is warmed at the UI priority').toBe(true);

  await loadByDoubleClick(page, TRACK);
  await expect.poll(async () => (await smoke.invokes('get_waveform_detail')).length).toBeGreaterThan(0);
  expect((await smoke.invokes('get_waveform_detail'))[0].trackId).toBe(warmed[0]);
});

test('without a high-resolution series the zoom view draws the analysis columns', async ({ smoke: { page } }) => {
  await expandPaused(page);
  await expect(page.locator('.ff-xp .waveform-display__zoom')).toHaveAttribute('data-bands', 'columns');
});

async function enableProgressiveDetail(page: Page, on: boolean) {
  await page.evaluate((flag) => (flag ? localStorage.setItem('ff.progressiveDetail', '1') : localStorage.removeItem('ff.progressiveDetail')), on);
  await page.reload();
  await page.locator('[data-track-row]').first().waitFor();
}

async function msToFirstZoomPaint(page: Page, detailMs: number) {
  await page.evaluate(() => ((window as unknown as SmokeWindow).__smokeDetail = 'hold'));
  await loadByDoubleClick(page, TRACK);
  const zoom = page.locator('.ff-xp .waveform-display__zoom');
  await page.locator('.ff-player-compact__expand-btn').click();
  const started = Date.now();
  await expect(zoom).toBeVisible();
  await expect(zoom).toHaveAttribute('data-bands', /.+/);
  const release = setTimeout(() => void page.evaluate(() => (window as unknown as SmokeWindow).__smokeReleaseDetail?.()), detailMs);
  await expect(zoom).not.toHaveAttribute('data-bands', /^(pending|none)$/);
  const painted = Date.now() - started;
  clearTimeout(release);
  await page.evaluate(() => (window as unknown as SmokeWindow).__smokeReleaseDetail?.());
  await expect(zoom).toHaveAttribute('data-bands', 'detail');
  return painted;
}

test('progressive detail shows the stored columns while the series is measured and swaps in the series in place', async ({ smoke: { page } }) => {
  await enableProgressiveDetail(page, true);
  await page.evaluate(() => ((window as unknown as SmokeWindow).__smokeDetail = 'hold'));
  await expandPaused(page);
  const zoom = page.locator('.ff-xp .waveform-display__zoom');
  await expect(zoom).toHaveAttribute('data-bands', 'columns');
  const box = await zoom.boundingBox();
  const shots = process.env.FF_SHOTS_DIR;
  if (shots) await page.locator('.ff-xp__wave').screenshot({ path: `${shots}/perf-detail-preview.png` });

  await page.evaluate(() => (window as unknown as SmokeWindow).__smokeReleaseDetail?.());
  await expect(zoom).toHaveAttribute('data-bands', 'detail');
  expect(await zoom.boundingBox()).toEqual(box);
  if (shots) await page.locator('.ff-xp__wave').screenshot({ path: `${shots}/perf-detail-final.png` });
});

test('time to the first painted zoom waveform with a cold detail of 450 ms, progressive detail off and on', async ({ smoke: { page } }) => {
  const COLD_DETAIL_MS = 450;
  const off = await msToFirstZoomPaint(page, COLD_DETAIL_MS);
  await enableProgressiveDetail(page, true);
  const on = await msToFirstZoomPaint(page, COLD_DETAIL_MS);
  console.log(`first painted zoom waveform: off ${off} ms, on ${on} ms`);
  expect(on).toBeLessThan(off);
});
