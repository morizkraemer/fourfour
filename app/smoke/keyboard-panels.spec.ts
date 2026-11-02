import type { Page } from '@playwright/test';
import { expect, test } from './app.ts';
import { SMOKE_STICKS_KEY } from './tauri-mock.ts';

const SANDISK = '/Volumes/SanDisk 32GB';

const stickRow = (page: Page, path: string) => page.locator(`[data-drop-stick-path="${path}"][data-drop-surface="row"] .ff-sbrow`);
const chip = (page: Page) => page.locator('[data-library-status]');

async function withSticks(page: Page) {
  await page.evaluate((key) => {
    localStorage.setItem(key, '1');
    localStorage.setItem('fourfour.organize.expanded', 'false');
  }, SMOKE_STICKS_KEY);
  await page.reload();
  await page.locator('[data-track-row]').first().waitFor();
  await expect(stickRow(page, SANDISK)).toBeVisible();
}

async function focusViaKeyboard(page: Page, target: ReturnType<Page['locator']>) {
  await page.keyboard.press('F6');
  await target.focus();
  await expect(target).toBeFocused();
}

test('stick: the sidebar row opens the stick view from the keyboard and its playlist rows open with Enter', async ({ smoke: { page } }) => {
  await withSticks(page);
  await focusViaKeyboard(page, stickRow(page, SANDISK));
  await page.keyboard.press('Enter');
  const view = page.locator('[data-stick-view]');
  await expect(view).toHaveAttribute('data-stick-view', SANDISK);
  await expect(view).toHaveAccessibleName('SanDisk 32GB');
  const row = view.locator('[data-stick-row="playlist"]').first();
  await row.focus();
  await expect(row).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.locator('[data-stick-playlist-header]')).toBeVisible();
});

test('stick: the remove button of a playlist row is reachable by Tab and named', async ({ smoke: { page } }) => {
  await withSticks(page);
  await focusViaKeyboard(page, stickRow(page, SANDISK));
  await page.keyboard.press('Enter');
  const view = page.locator('[data-stick-view]');
  const row = view.locator('[data-stick-row="playlist"]').first();
  await row.focus();
  await page.keyboard.press('Tab');
  await expect(page.locator(':focus')).toHaveAttribute('aria-label', /^Take .* off this stick$/);
});

test('library activity: Enter opens the panel, focus moves in, Escape returns focus to the chip', async ({ smoke: { page } }) => {
  await focusViaKeyboard(page, chip(page));
  await expect(chip(page)).toHaveAttribute('aria-haspopup', 'menu');
  await expect(chip(page)).toHaveAttribute('aria-expanded', 'false');
  await page.keyboard.press('Enter');
  const menu = page.locator('[data-library-activity-menu]');
  await expect(menu).toBeVisible();
  await expect(chip(page)).toHaveAttribute('aria-expanded', 'true');
  expect(await menu.evaluate((el) => el.contains(document.activeElement))).toBe(true);
  await page.keyboard.press('Escape');
  await expect(menu).toHaveCount(0);
  await expect(chip(page)).toBeFocused();
});

test('BPM popup: Escape closes it and returns focus to the pencil', async ({ smoke: { page } }) => {
  await page.locator('[data-track-row]').first().dblclick();
  await expect(page.locator('.ff-player-compact__meta-title')).toHaveText('Smoke Track 01');
  await page.locator('.ff-player-compact__expand-btn').click();
  const pen = page.getByRole('button', { name: 'Edit track BPM' });
  await pen.focus();
  await page.keyboard.press('Enter');
  const popup = page.getByRole('dialog', { name: 'Track BPM' });
  await expect(popup).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(popup).toHaveCount(0);
  await expect(pen).toBeFocused();
});

test('player: the focused CUE button answers Space, Space and Enter on Play toggle once each', async ({ smoke }) => {
  const { page } = smoke;
  await page.locator('[data-track-row]').first().dblclick();
  await expect(page.locator('.ff-player-compact__meta-title')).toHaveText('Smoke Track 01');
  await page.keyboard.press('Space');
  await expect(page.getByRole('button', { name: 'Play', exact: true })).toBeVisible();
  const cueButton = page.locator('.ff-player-compact__cue-btn');
  await page.keyboard.press('F6');
  await cueButton.focus();
  const playsBefore = (await smoke.invokes('playback_play')).length;
  await page.keyboard.down('Space');
  await expect.poll(async () => (await smoke.invokes('playback_play')).length).toBeGreaterThan(playsBefore);
  await page.keyboard.up('Space');
  await expect(page.getByRole('button', { name: 'Play', exact: true })).toBeVisible();
  const play = page.getByRole('button', { name: /^(Play|Pause)$/ });
  await play.focus();
  const before = await play.getAttribute('aria-label');
  await page.keyboard.press('Space');
  await expect(play).not.toHaveAttribute('aria-label', before!);
  const after = await play.getAttribute('aria-label');
  await page.keyboard.press('Enter');
  await expect(play).not.toHaveAttribute('aria-label', after!);
});
