import type { Page } from '@playwright/test';
import { expect, loadByDoubleClick, test } from './app.ts';

const TRACK = 'Smoke Track 03';
const SHOTS = process.env.SHOTS_DIR;

const bpmPen = (page: Page) => page.getByRole('button', { name: 'Edit track BPM' });
const keyPen = (page: Page) => page.getByRole('button', { name: 'Edit track key' });
const bpmPopup = (page: Page) => page.getByRole('dialog', { name: 'Track BPM' });
const keyPopup = (page: Page) => page.getByRole('dialog', { name: 'Track key' });
const bpmInput = (page: Page) => bpmPopup(page).getByRole('textbox', { name: 'Track BPM' });
const save = (page: Page) => bpmPopup(page).getByRole('button', { name: 'Save', exact: true });

async function expandWithTrack(page: Page) {
  await loadByDoubleClick(page, TRACK);
  await page.locator('.ff-player-compact__expand-btn').click();
  await expect(page.locator('.ff-xp')).toBeVisible();
}

async function expectInsideWindow(page: Page, popup: ReturnType<typeof bpmPopup>) {
  const box = (await popup.boundingBox())!;
  const view = page.viewportSize()!;
  expect(box.x).toBeGreaterThanOrEqual(0);
  expect(box.y).toBeGreaterThanOrEqual(0);
  expect(box.x + box.width).toBeLessThanOrEqual(view.width);
  expect(box.y + box.height).toBeLessThanOrEqual(view.height);
}

test('the BPM pencil opens a popup: Save waits for a change, Enter saves, Esc cancels', async ({ smoke }) => {
  const { page } = smoke;
  await expandWithTrack(page);
  await bpmPen(page).click();
  await expect(bpmPopup(page)).toBeVisible();
  await expect(bpmInput(page)).toHaveValue('126.00');
  await expect(bpmInput(page)).toBeFocused();
  await expect(save(page)).toBeDisabled();
  if (SHOTS) await page.screenshot({ path: `${SHOTS}/bpm-popup-open.png` });

  await page.keyboard.type('128');
  await expect(save(page)).toBeEnabled();
  if (SHOTS) await page.screenshot({ path: `${SHOTS}/bpm-popup-typing.png` });
  await page.keyboard.press('Escape');
  await expect(bpmPopup(page)).toHaveCount(0);
  expect(await smoke.invokes('rescale_beat_grid')).toEqual([]);
  await expect(page.locator('.ff-xp')).toBeVisible();

  await bpmPen(page).click();
  await page.keyboard.type('128');
  await page.keyboard.press('Enter');
  await expect(bpmPopup(page)).toHaveCount(0);
  await expect.poll(async () => (await smoke.invokes('rescale_beat_grid')).map((c) => c.bpm)).toEqual([128]);
});

test('the BPM popup halves, doubles and taps', async ({ smoke }) => {
  const { page } = smoke;
  await expandWithTrack(page);
  await bpmPen(page).click();
  await bpmPopup(page).getByRole('button', { name: 'Halve track BPM' }).click();
  await expect(bpmInput(page)).toHaveValue('63.00');
  await bpmPopup(page).getByRole('button', { name: 'Double track BPM' }).click();
  await expect(bpmInput(page)).toHaveValue('126.00');
  await expect(save(page)).toBeDisabled();

  const tap = bpmPopup(page).getByRole('button', { name: 'Tap', exact: true });
  for (let i = 0; i < 5; i++) {
    await tap.click();
    await page.waitForTimeout(400);
  }
  const tapped = Number(await bpmInput(page).inputValue());
  expect(tapped).toBeGreaterThan(100);
  expect(tapped).toBeLessThan(170);
  expect(tapped).not.toBe(126);
  await save(page).click();
  await expect.poll(async () => (await smoke.invokes('rescale_beat_grid')).length).toBe(1);
});

test('the key popup opens on the tab of the current key and marks it and its neighbours', async ({ smoke }) => {
  const { page } = smoke;
  await expandWithTrack(page);
  await keyPen(page).click();
  const popup = keyPopup(page);
  await expect(popup).toBeVisible();
  await expect(popup.getByRole('button', { name: 'Minor' })).toHaveClass(/--on/);
  await expect(popup.locator('[data-key]')).toHaveCount(12);
  await expect(popup.locator('.ff-keypop__key--on')).toHaveText('5A· Cm');
  await expect(popup.locator('.ff-keypop__key--fit')).toHaveText(['4A· Fm', '6A· Gm']);
  await expect(popup.locator('[data-key="5A"]')).toBeFocused();
  const box = (await popup.boundingBox())!;
  expect(box.width).toBeLessThanOrEqual(290);
  expect(box.height).toBeLessThanOrEqual(270);
  if (SHOTS) await page.screenshot({ path: `${SHOTS}/key-popup-open.png` });

  await popup.getByRole('button', { name: 'Major' }).click();
  await expect(popup.locator('.ff-keypop__key--fit')).toHaveCount(1);
  await expect(popup.locator('[data-key="5B"]')).toHaveClass(/--fit/);
  await popup.locator('[data-key="6B"]').hover();
  if (SHOTS) await page.screenshot({ path: `${SHOTS}/key-popup-major-hover.png` });
});

test('arrow keys move over the key grid, Enter saves and closes; clicking a key does too', async ({ smoke }) => {
  const { page } = smoke;
  await expandWithTrack(page);
  await keyPen(page).click();
  const popup = keyPopup(page);
  await expect(popup.locator('[data-key="5A"]')).toBeFocused();
  await page.keyboard.press('ArrowRight');
  await expect(popup.locator('[data-key="6A"]')).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await expect(popup.locator('[data-key="9A"]')).toBeFocused();
  await page.keyboard.press('ArrowLeft');
  await page.keyboard.press('ArrowUp');
  await expect(popup.locator('[data-key="5A"]')).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Enter');
  await expect(popup).toHaveCount(0);
  await expect.poll(async () => (await smoke.invokes('set_track_key')).map((c) => c.key)).toEqual(['8A']);

  await keyPen(page).click();
  await keyPopup(page).getByRole('button', { name: 'Major' }).click();
  await keyPopup(page).locator('[data-key="3B"]').click();
  await expect(keyPopup(page)).toHaveCount(0);
  await expect.poll(async () => (await smoke.invokes('set_track_key')).map((c) => c.key)).toEqual(['8A', '3B']);
});

test('Reset to analyzed releases the hand-set values and analyses the track again', async ({ smoke }) => {
  const { page } = smoke;
  await expandWithTrack(page);
  await keyPen(page).click();
  await expect(keyPopup(page).getByRole('button', { name: 'Reset to analyzed' })).toBeDisabled();
  await keyPopup(page).locator('[data-key="6A"]').click();
  await expect(keyPopup(page)).toHaveCount(0);
  await keyPen(page).click();
  await keyPopup(page).getByRole('button', { name: 'Reset to analyzed' }).click();
  await expect(keyPopup(page)).toHaveCount(0);
  await expect.poll(async () => (await smoke.invokes('release_user_edits')).length).toBe(1);
  await expect.poll(async () => (await smoke.invokes('analyze_track_ids')).length).toBe(1);
});

test('click outside closes a popup and opening the other pencil swaps them', async ({ smoke: { page } }) => {
  await expandWithTrack(page);
  await bpmPen(page).click();
  await expect(bpmPopup(page)).toBeVisible();
  await keyPen(page).click();
  await expect(bpmPopup(page)).toHaveCount(0);
  await expect(keyPopup(page)).toBeVisible();
  await page.mouse.click(600, 600);
  await expect(keyPopup(page)).toHaveCount(0);
  await expect(page.locator('.ff-xp')).toBeVisible();
});

for (const width of [900, 1600]) {
  test(`both popups stay inside the window at ${width} px`, async ({ smoke: { page } }) => {
    await expandWithTrack(page);
    await page.setViewportSize({ width, height: 800 });
    await bpmPen(page).click();
    await expectInsideWindow(page, bpmPopup(page));
    await keyPen(page).click();
    await expectInsideWindow(page, keyPopup(page));
    if (SHOTS) await page.screenshot({ path: `${SHOTS}/key-popup-${width}.png` });
  });
}
