import type { Page } from '@playwright/test';
import { expect, loadByDoubleClick, rowByTitle, test } from './app.ts';

const TRACK = 'Smoke Track 01';
const SHOTS = process.env.FF_SHOTS;

const inspector = (page: Page) => page.locator('.ff-inspector');
const playButton = (page: Page) => page.locator('.ff-xp__play');

async function loadAndExpand(page: Page) {
  await rowByTitle(page, TRACK).click();
  await loadByDoubleClick(page, TRACK);
  await page.locator('.ff-player-compact__expand-btn').click();
  await expect(page.locator('.ff-xp')).toBeVisible();
  await expect(inspector(page)).toBeVisible();
}

async function pause(page: Page) {
  if ((await playButton(page).getAttribute('aria-label')) === 'Pause') await playButton(page).click();
  await expect(playButton(page)).toHaveAttribute('aria-label', 'Play');
}

test('the inspector has a close button that closes it', async ({ smoke: { page } }) => {
  await rowByTitle(page, TRACK).click();
  await expect(inspector(page)).toBeVisible();
  const close = page.getByRole('button', { name: 'Close inspector' });
  const box = (await close.boundingBox())!;
  expect(box.width).toBeGreaterThanOrEqual(40);
  expect(box.height).toBeGreaterThanOrEqual(40);
  const pane = (await inspector(page).boundingBox())!;
  expect(box.x + box.width).toBeGreaterThan(pane.x + pane.width - 8);
  expect(box.y).toBeLessThan(pane.y + 8);
  if (SHOTS) await inspector(page).screenshot({ path: `${SHOTS}/inspector-close-button.png` });
  await close.click();
  await expect(inspector(page)).toHaveCount(0);
});

for (const size of [
  { width: 1440, height: 900 },
  { width: 1280, height: 800 },
]) {
  test(`the inspector scrolls to its last item with the big player open at ${size.width}x${size.height}`, async ({ smoke: { page } }) => {
    await page.setViewportSize(size);
    await loadAndExpand(page);
    const pane = inspector(page);
    const player = (await page.locator('.ff-player-module').boundingBox())!;
    const paneBox = (await pane.boundingBox())!;
    expect(paneBox.y + paneBox.height).toBeLessThanOrEqual(player.y + 1);

    await page.mouse.move(paneBox.x + paneBox.width / 2, paneBox.y + paneBox.height / 2);
    await page.mouse.wheel(0, 2000);
    await expect.poll(() => pane.evaluate((el) => el.scrollTop + el.clientHeight >= el.scrollHeight - 1)).toBe(true);
    const last = (await pane.locator(':scope > :last-child').boundingBox())!;
    const after = (await pane.boundingBox())!;
    expect(last.y + last.height).toBeLessThanOrEqual(after.y + after.height + 1);
    if (SHOTS) await page.screenshot({ path: `${SHOTS}/inspector-scrolled-${size.width}x${size.height}.png` });
    await page.mouse.wheel(0, -2000);
    await expect.poll(() => pane.evaluate((el) => el.scrollTop)).toBe(0);
  });
}

test('Shift+1 to Shift+8 set an empty hot cue and fire a set one, like the pads', async ({ smoke }) => {
  const { page } = smoke;
  await loadAndExpand(page);
  await pause(page);
  const saves = async () => (await smoke.invokes('set_cues')).length;
  const before = await saves();

  await page.keyboard.press('Shift+2');
  await expect.poll(saves).toBeGreaterThan(before);
  const saved = (await smoke.invokes('set_cues')).at(-1)!.cues as { hot_cue_number: number }[];
  expect(saved.map((c) => c.hot_cue_number)).toContain(2);
  await expect(page.locator('.ff-xp__pad[data-pad="B"]')).toHaveClass(/ff-xp__pad--set/);

  await expect(playButton(page)).toHaveAttribute('aria-label', 'Play');
  await page.keyboard.press('Shift+1');
  await expect(playButton(page)).toHaveAttribute('aria-label', 'Pause');
  expect(await saves()).toBe(before + 1);
});

test('keys 1 to 8 do nothing while typing in a text field', async ({ smoke }) => {
  const { page } = smoke;
  await loadAndExpand(page);
  await pause(page);
  const before = (await smoke.invokes('set_cues')).length;
  const search = page.getByPlaceholder('Search tracks and commands');
  await search.click();
  await page.keyboard.type('1234');
  await expect(search).toHaveValue('1234');
  await expect(playButton(page)).toHaveAttribute('aria-label', 'Play');
  expect((await smoke.invokes('set_cues')).length).toBe(before);
});

test('firing a memory cue from the inspector while paused jumps and starts playback', async ({ smoke: { page } }) => {
  await loadAndExpand(page);
  await pause(page);
  await page.locator('.ff-inspector__cue-main').first().click();
  await expect(playButton(page)).toHaveAttribute('aria-label', 'Pause');
});

test('firing a memory cue chip in the big player while paused starts playback', async ({ smoke: { page } }) => {
  await loadAndExpand(page);
  await pause(page);
  await page.locator('[data-memory-chip]').first().click();
  await expect(playButton(page)).toHaveAttribute('aria-label', 'Pause');
});
