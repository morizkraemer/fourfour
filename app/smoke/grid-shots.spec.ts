import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { expect, loadByDoubleClick, test } from './app.ts';

const OUT = process.env.FF_GRID_SHOTS;

test.use({ deviceScaleFactor: 2 });

test('grid review shots: big player at three zoom levels, small player', async ({ smoke: { page } }) => {
  test.skip(!OUT, 'set FF_GRID_SHOTS=<dir>');
  mkdirSync(OUT!, { recursive: true });
  await loadByDoubleClick(page, 'Smoke Track 03');
  const compact = page.locator('.ff-player-compact').first();
  const compactOverview = (await compact.locator('.waveform-display__zoom').boundingBox())!;
  await page.mouse.click(compactOverview.x + compactOverview.width * 0.37, compactOverview.y + compactOverview.height / 2);
  if ((await compact.getByRole('button', { name: 'Pause' }).count()) > 0) await page.keyboard.press('Space');
  await page.waitForTimeout(400);
  await page.screenshot({ path: join(OUT!, 'small-player.png'), clip: (await compact.boundingBox())! });

  await page.locator('.ff-player-compact__expand-btn').click();
  await expect(page.locator('.ff-xp')).toBeVisible();
  const display = page.locator('.ff-xp .waveform-display');
  const zoom = (await page.locator('.ff-xp .waveform-display__zoom').boundingBox())!;
  for (const [label, wheel] of [['big-zoomed-out', 1200], ['big-default', -1200], ['big-zoomed-in', -700]] as const) {
    await page.mouse.move(zoom.x + zoom.width / 2, zoom.y + zoom.height / 2);
    await page.mouse.wheel(0, wheel);
    await page.waitForTimeout(500);
    await page.screenshot({ path: join(OUT!, `${label}.png`), clip: (await display.boundingBox())! });
  }
});
