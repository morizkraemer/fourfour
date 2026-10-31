import { mkdirSync } from 'node:fs';
import { homedir } from 'node:os';
import { resolve } from 'node:path';
import type { Locator, Page } from '@playwright/test';
import { expect, sidebarPlaylist, test } from './app.ts';
import { SMOKE_STICKS_KEY } from './tauri-mock.ts';

const SHOTS = process.env.FF_SHOTS_DIR ?? resolve(homedir(), '.cache/b10-shots');
mkdirSync(SHOTS, { recursive: true });

const SANDISK = '/Volumes/SanDisk 32GB';
const BACKUP = '/Volumes/BACKUP 32GB';
const PANORAMA = '/Volumes/PANORAMA 8GB';

const stickRow = (page: Page, path: string) => page.locator(`[data-drop-stick-path="${path}"][data-drop-surface="row"]`);
const view = (page: Page) => page.locator('[data-stick-view]');
const strip = (page: Page) => page.locator('[data-put-strip]');
const listRow = (page: Page, name: string) => view(page).locator('.ff-stickview__row').filter({ hasText: name });

async function withSticks(page: Page, width = 1440, height = 900) {
  await page.setViewportSize({ width, height });
  await page.evaluate((key) => {
    localStorage.setItem(key, '1');
    localStorage.setItem('fourfour.organize.expanded', 'false');
  }, SMOKE_STICKS_KEY);
  await page.reload();
  await page.locator('[data-track-row]').first().waitFor();
  await expect(stickRow(page, SANDISK)).toBeVisible();
}

async function openStick(page: Page, path: string) {
  await stickRow(page, path).locator('.ff-sbrow__label').click();
  await expect(view(page)).toHaveAttribute('data-stick-view', path);
}

async function drag(page: Page, from: Locator, to: () => Locator, during?: () => Promise<void>) {
  const a = (await from.boundingBox())!;
  await page.mouse.move(a.x + 20, a.y + a.height / 2);
  await page.mouse.down();
  await page.mouse.move(a.x + 40, a.y + a.height / 2 + 10, { steps: 4 });
  const b = (await to().boundingBox())!;
  await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2, { steps: 12 });
  await during?.();
  await page.mouse.up();
}

async function shot(page: Page, name: string, target?: Locator) {
  await (target ?? page).screenshot({ path: `${SHOTS}/${name}.png` });
}

test('stick on the left: adaptive header, playlists with Synced mark, stick-only group', async ({ smoke: { page } }) => {
  await withSticks(page);
  await openStick(page, SANDISK);
  const header = page.locator(`[data-stick-header="${SANDISK}"]`);
  await expect(header).toContainText('SanDisk 32GB');
  await expect(header).toContainText('22.2 GB free');
  await expect(header).toContainText('Synced 2 min ago');
  await expect(listRow(page, 'Peak Time').locator('[data-synced-mark]')).toBeVisible();
  await expect(page.locator('[data-only-on-stick]')).toBeVisible();
  await expect(view(page).locator('[data-stick-row="foreign"]')).toHaveCount(2);
  await expect(strip(page)).toHaveCount(0);
  await shot(page, '01-stick-left-full');
  await page.setViewportSize({ width: 390 * 2, height: 844 });
  await shot(page, '01b-stick-left-half-width');
});

test('panel closed: a playlist goes on the stick through the Put on stick strip', async ({ smoke }) => {
  const { page } = smoke;
  await withSticks(page);
  await openStick(page, SANDISK);
  await drag(page, sidebarPlaylist(page, 'Warmup'), () => strip(page), async () => {
    await expect(strip(page)).toContainText('Put on SanDisk 32GB · Keep updated');
    await shot(page, '02-put-strip-hover');
  });
  expect(await smoke.invokes('stick_add_playlists')).toEqual([{ path: SANDISK, playlistIds: [2] }]);
  await expect(listRow(page, 'Warmup').locator('[data-synced-mark]')).toBeVisible();
  await expect(strip(page)).toHaveCount(0);
});

test('panel closed: dropping on the stick view itself does nothing', async ({ smoke }) => {
  const { page } = smoke;
  await withSticks(page);
  await openStick(page, SANDISK);
  await drag(page, sidebarPlaylist(page, 'Warmup'), () => view(page).locator('[data-only-on-stick]'), async () => {
    await expect(strip(page)).toBeVisible();
    await expect(strip(page)).toHaveText('Put on SanDisk 32GB');
  });
  expect(await smoke.invokes('stick_add_playlists')).toEqual([]);
});

test('copy a playlist to another stick: Keep updated stays on', async ({ smoke }) => {
  const { page } = smoke;
  await withSticks(page);
  await openStick(page, SANDISK);
  await drag(page, listRow(page, 'Peak Time'), () => stickRow(page, BACKUP), async () => {
    await expect(page.locator('.ff-stickdroptip')).toHaveText('Copy to BACKUP 32GB · Keep updated');
    await shot(page, '03-copy-to-other-stick');
  });
  expect(await smoke.invokes('stick_add_playlists')).toEqual([{ path: BACKUP, playlistIds: [1] }]);
  expect(await smoke.invokes('stick_set_keep_updated')).toEqual([]);
  await openStick(page, BACKUP);
  await expect(listRow(page, 'Peak Time').locator('[data-synced-mark]')).toBeVisible();
  await expect(listRow(page, 'Peak Time')).toContainText('follows your library');
});

test('stick playlist: Keep updated switch, off shows the date', async ({ smoke }) => {
  const { page } = smoke;
  await withSticks(page);
  await openStick(page, SANDISK);
  await listRow(page, 'Peak Time').click();
  const head = page.locator('[data-stick-playlist-header]');
  await expect(head).toContainText('SanDisk 32GB');
  await expect(head).toContainText('Peak Time');
  const toggle = head.getByRole('switch', { name: 'Keep updated' });
  await expect(toggle).toHaveAttribute('aria-checked', 'true');
  await expect(head.locator('[data-keep-note]')).toHaveText('synced 2 min ago');
  await shot(page, '04-stick-playlist-keep-updated-on');
  await toggle.click();
  await expect(toggle).toHaveAttribute('aria-checked', 'false');
  await expect(head.locator('[data-keep-note]')).toContainText('as of');
  expect(await smoke.invokes('stick_set_keep_updated')).toEqual([{ path: SANDISK, playlistId: 1, keepUpdated: false }]);
  await shot(page, '05-stick-playlist-keep-updated-off');
  await head.getByRole('button', { name: 'SanDisk 32GB' }).click();
  await expect(listRow(page, 'Peak Time').locator('[data-synced-mark]')).toHaveCount(0);
  await expect(listRow(page, 'Peak Time')).toContainText('as of');
});

test('engine without Keep updated: the switch stays on and is disabled', async ({ smoke }) => {
  const { page } = smoke;
  await page.evaluate(() => localStorage.setItem('smoke.sticks.oldEngine', '1'));
  await withSticks(page);
  await openStick(page, SANDISK);
  await listRow(page, 'Peak Time').click();
  const toggle = page.locator('[data-stick-playlist-header]').getByRole('switch', { name: 'Keep updated' });
  await expect(toggle).toHaveAttribute('aria-checked', 'true');
  await expect(toggle).toHaveAttribute('aria-disabled', 'true');
  await toggle.click({ force: true });
  expect(await smoke.invokes('stick_set_keep_updated')).toEqual([]);
});

test('sync asks when tracks come off the stick', async ({ smoke }) => {
  const { page } = smoke;
  await withSticks(page);
  await openStick(page, SANDISK);
  await page.locator(`[data-stick-header="${SANDISK}"]`).getByRole('button', { name: 'Sync' }).click();
  const plan = page.locator('[data-stick-plan]');
  await expect(plan.locator('[data-plan-remove]')).toContainText('2 tracks come off the stick');
  await expect(plan.locator('[data-plan-foreign]')).toContainText('2 tracks put there by other apps');
  await shot(page, '06-removal-ask');
  expect(await smoke.invokes('sync_usb_many')).toEqual([]);
  await page.getByRole('button', { name: 'Sync', exact: true }).last().click();
  await expect(page.locator(`[data-stick-header="${SANDISK}"] [data-stick-status]`)).toContainText('Safe to eject');
  expect(await smoke.invokes('sync_usb_many')).toHaveLength(1);
});

test("sync stops when it won't fit", async ({ smoke }) => {
  const { page } = smoke;
  await withSticks(page);
  await openStick(page, PANORAMA);
  await page.locator(`[data-stick-header="${PANORAMA}"]`).getByRole('button', { name: 'Sync' }).click();
  await expect(page.locator('[data-plan-wontfit]')).toContainText('Nothing was written');
  await expect(page.getByText("Won't fit on PANORAMA 8GB")).toBeVisible();
  await shot(page, '07-wont-fit');
  await page.getByRole('button', { name: 'OK' }).click();
  expect(await smoke.invokes('sync_usb_many')).toEqual([]);
});

test('stick in the side panel: compact header, drop label on the board', async ({ smoke }) => {
  const { page } = smoke;
  await withSticks(page, 1280, 800);
  await stickRow(page, BACKUP).hover();
  await stickRow(page, BACKUP).locator('.ff-sbrow__action').click();
  const board = page.locator('[data-stick-board]');
  await expect(board.locator(`[data-stick-header="${BACKUP}"]`)).toContainText('30.1 GB free');
  await shot(page, '08-panel-stick-1280', page.locator('.ff-organize--expanded'));
  await drag(page, sidebarPlaylist(page, 'Peak Time'), () => board, async () => {
    await expect(board.locator('[data-drop-label]')).toHaveText('Put on BACKUP 32GB · Keep updated');
    await shot(page, '09-panel-drop-label');
  });
  expect(await smoke.invokes('stick_add_playlists')).toEqual([{ path: BACKUP, playlistIds: [1] }]);
});
