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
  const setClock = (ms: number) => page.evaluate((now) => ((performance as { now: () => number }).now = () => now), ms);
  for (let i = 0; i < 3; i++) {
    await setClock(i * 500);
    await tap.click();
    await expect(bpmInput(page)).toHaveValue('126.00');
  }
  await setClock(1500);
  await tap.click();
  await expect(bpmInput(page)).toHaveValue('120.00');
  await setClock(1500 + 2500);
  await tap.click();
  await expect(bpmInput(page)).toHaveValue('120.00');
  await setClock(4000 + 400);
  await tap.click();
  await setClock(4000 + 800);
  await tap.click();
  await setClock(4000 + 1200);
  await tap.click();
  await expect(bpmInput(page)).toHaveValue('150.00');
  await save(page).click();
  await expect.poll(async () => (await smoke.invokes('rescale_beat_grid')).length).toBe(1);
});

test('the key popup is two columns with the current key highlighted and no tabs', async ({ smoke }) => {
  const { page } = smoke;
  await expandWithTrack(page);
  await keyPen(page).click();
  const popup = keyPopup(page);
  await expect(popup).toBeVisible();
  await expect(popup.getByRole('button', { name: 'Minor' })).toHaveCount(0);
  await expect(popup.getByRole('button', { name: 'Major' })).toHaveCount(0);
  await expect(popup.locator('[data-key]')).toHaveCount(24);
  await expect(popup.locator('.ff-keypop__key--on')).toHaveCount(1);
  await expect(popup.locator('.ff-keypop__key--on')).toHaveText('5ACm');
  await expect(popup.locator('[data-key="5A"]')).toBeFocused();
  const fill = await popup.locator('.ff-keypop__key--on').evaluate((el) => getComputedStyle(el).backgroundColor);
  expect(fill).not.toBe('rgba(0, 0, 0, 0)');
  const plain = await popup.locator('[data-key="6A"]').evaluate((el) => getComputedStyle(el).backgroundColor);
  expect(plain).toBe('rgba(0, 0, 0, 0)');
  const box = (await popup.boundingBox())!;
  expect(box.width).toBeCloseTo(228, 0);
  expect(box.height).toBeLessThanOrEqual(300);
  if (SHOTS) await page.screenshot({ path: `${SHOTS}/key-popup-open.png` });
  await popup.locator('[data-key="6B"]').hover();
  if (SHOTS) await page.screenshot({ path: `${SHOTS}/key-popup-hover.png` });
});

test('arrow keys move within and across the columns, Enter saves and closes; clicking either column saves', async ({ smoke }) => {
  const { page } = smoke;
  await expandWithTrack(page);
  await keyPen(page).click();
  const popup = keyPopup(page);
  await expect(popup.locator('[data-key="5A"]')).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await expect(popup.locator('[data-key="6A"]')).toBeFocused();
  await page.keyboard.press('ArrowRight');
  await expect(popup.locator('[data-key="6B"]')).toBeFocused();
  await page.keyboard.press('ArrowUp');
  await expect(popup.locator('[data-key="5B"]')).toBeFocused();
  await page.keyboard.press('ArrowLeft');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown');
  await expect(popup.locator('[data-key="8A"]')).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(popup).toHaveCount(0);
  await expect.poll(async () => (await smoke.invokes('set_track_key')).map((c) => c.key)).toEqual(['8A']);

  await keyPen(page).click();
  await keyPopup(page).locator('[data-key="3B"]').click();
  await expect(keyPopup(page)).toHaveCount(0);
  await expect.poll(async () => (await smoke.invokes('set_track_key')).map((c) => c.key)).toEqual(['8A', '3B']);

  await keyPen(page).click();
  await keyPopup(page).locator('[data-key="2A"]').click();
  await expect(keyPopup(page)).toHaveCount(0);
  await expect.poll(async () => (await smoke.invokes('set_track_key')).map((c) => c.key)).toEqual(['8A', '3B', '2A']);
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
  await expect.poll(async () => await smoke.invokes('release_user_field')).toEqual([{ trackId: 3, field: 'key' }]);
  await expect.poll(async () => (await smoke.invokes('analyze_track_ids')).length).toBe(1);
  expect(await smoke.invokes('release_user_edits')).toEqual([]);
});

test('the BPM popup resets only the BPM', async ({ smoke }) => {
  const { page } = smoke;
  await expandWithTrack(page);
  await bpmPen(page).click();
  await expect(bpmPopup(page).getByRole('button', { name: 'Reset to analyzed' })).toBeDisabled();
  await page.keyboard.type('130');
  await page.keyboard.press('Enter');
  await bpmPen(page).click();
  await bpmPopup(page).getByRole('button', { name: 'Reset to analyzed' }).click();
  await expect.poll(async () => await smoke.invokes('release_user_field')).toEqual([{ trackId: 3, field: 'bpm' }]);
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
