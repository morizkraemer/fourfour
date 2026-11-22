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
const view = (page: Page) => page.locator('.ff-browse__slot--table [data-stick-view]');
const inspector = (page: Page) => page.locator('[data-stick-inspector]');
const stickStatus = (page: Page, path: string) => stickRow(page, path).locator('[data-stick-status]');
const problem = (page: Page) => page.locator('[data-stick-problem]');
const trackRows = (page: Page) => view(page).locator('[data-stick-track]');
const trackTitles = (page: Page) => trackRows(page).locator('.ff-stickview__t').allTextContents();
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

test('stick on the left: info in the inspector, playlists with Synced mark, stick-only group', async ({ smoke: { page } }) => {
  await withSticks(page);
  await openStick(page, SANDISK);
  await expect(page.locator('[data-stick-header]')).toHaveCount(0);
  await expect(inspector(page)).toHaveCount(1);
  await expect(inspector(page)).toContainText('SanDisk 32GB');
  await expect(inspector(page).locator('[data-fig="free"]')).toContainText('22.2 GB');
  await expect(inspector(page).locator('[data-stick-capacity]')).toContainText('7.6 GB used of 29.8 GB');
  await expect(inspector(page).locator('[data-stick-status]')).toHaveAttribute('data-stick-status', 'pending');
  await expect(inspector(page).locator('[data-stick-detail]')).toHaveText('2 to remove. Written on the next sync.');
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

test('engine without Keep updated: plain text instead of a switch that cannot move', async ({ smoke }) => {
  const { page } = smoke;
  await page.evaluate(() => localStorage.setItem('smoke.sticks.oldEngine', '1'));
  await withSticks(page);
  await openStick(page, SANDISK);
  await listRow(page, 'Peak Time').click();
  const head = page.locator('[data-stick-playlist-header]');
  await expect(head.getByRole('switch')).toHaveCount(0);
  await expect(head.locator('[data-keep-updated-fixed]')).toHaveText('Follows your library');
  await trackRows(page).first().click({ button: 'right' });
  await expect(page.getByRole('menuitem', { name: 'Remove from this playlist on the stick' })).toBeDisabled();
});

test('status dots: none when in sync, amber when changes wait, red when it will not fit', async ({ smoke: { page } }) => {
  await withSticks(page);
  await expect(stickStatus(page, SANDISK)).toHaveAttribute('data-stick-status', 'pending');
  await expect(stickStatus(page, PANORAMA)).toHaveAttribute('data-stick-status', 'wont-fit');
  await expect(stickStatus(page, BACKUP)).toHaveAttribute('data-stick-status', 'idle');
  await expect(stickStatus(page, BACKUP).locator('[data-stick-dot]')).toHaveCount(0);
  await expect(stickStatus(page, SANDISK).locator('[data-stick-dot]')).toHaveAttribute('data-stick-dot', 'waiting');
  await expect(stickStatus(page, PANORAMA).locator('[data-stick-dot]')).toHaveAttribute('data-stick-dot', 'problem');
  await openStick(page, BACKUP);
  await expect(stickStatus(page, BACKUP).locator('[data-stick-dot]')).toHaveCount(0);
  await shot(page, '10-sidebar-status-dots', page.locator('.ff-sidebar__bottom'));
});

test('top bar pill shows only a stick that needs attention and opens its inspector', async ({ smoke }) => {
  const { page } = smoke;
  await withSticks(page);
  const chip = page.locator('[data-stick-chip]');
  await expect(chip).toHaveAttribute('data-stick-chip', 'wont-fit');
  await expect(chip).toContainText('PANORAMA 8GB Too much');
  await chip.click();
  await expect(problem(page)).toHaveAttribute('data-stick-problem', 'wont-fit');
  await page.getByRole('button', { name: 'Show stick' }).click();
  await expect(problem(page)).toHaveCount(0);
  await expect(inspector(page)).toHaveAttribute('data-stick-inspector', PANORAMA);
  await inspector(page).locator('[data-stick-problem-open]').click();
  await expect(problem(page)).toBeVisible();
  await expect(page.getByRole('button', { name: 'Show stick' })).toHaveCount(0);
  await page.keyboard.press('Escape');
  await stickRow(page, PANORAMA).click({ button: 'right' });
  await smoke.answerDialogs(true);
  await page.getByRole('menuitem', { name: 'Eject' }).click();
  await expect(chip).toHaveAttribute('data-stick-chip', 'pending');
  await expect(chip).toContainText('SanDisk 32GB 2 changes waiting');
  await openStick(page, SANDISK);
  await inspector(page).getByRole('button', { name: 'Sync now' }).click();
  await page.getByRole('button', { name: 'Sync', exact: true }).last().click();
  await expect(inspector(page).locator('[data-stick-status]')).toHaveAttribute('data-stick-status', 'idle');
  await expect(chip).toHaveCount(0);
});

test('a stick problem is one warning symbol; clicking it says what is wrong and what to do', async ({ smoke }) => {
  const { page } = smoke;
  await withSticks(page);
  const warning = stickStatus(page, PANORAMA);
  await expect(warning).toHaveAttribute('data-stick-status', 'wont-fit');
  expect(await warning.evaluate((el) => el.tagName)).toBe('BUTTON');
  await warning.click();
  await expect(page.locator('[data-stick-view]')).toHaveCount(0);
  await expect(page.locator('[data-stick-problem-title]')).toHaveText('Too much for PANORAMA 8GB');
  await expect(problem(page)).toContainText('too much: the new tracks need');
  await expect(problem(page)).toContainText('Take a playlist off, or free up space.');
  await shot(page, '11-stick-problem-popover');
  await problem(page).locator('[data-stick-problem-playlist="1"]').getByRole('button', { name: 'Take off' }).click();
  expect(await smoke.invokes('stick_remove_playlists')).toEqual([{ path: PANORAMA, playlistIds: [1] }]);
  await expect(problem(page)).toHaveCount(0);
  await expect(stickStatus(page, PANORAMA)).toHaveAttribute('data-stick-status', 'pending');
});

test('a failed sync shows the warning with the error and a retry', async ({ smoke }) => {
  const { page } = smoke;
  await withSticks(page);
  await openStick(page, BACKUP);
  await drag(page, sidebarPlaylist(page, 'Warmup'), () => strip(page));
  await page.evaluate(() => ((window as unknown as { __smokeFailSync: string | null }).__smokeFailSync = 'Cannot open /Volumes/BACKUP 32GB: gone'));
  await inspector(page).getByRole('button', { name: 'Sync now' }).click();
  await expect(stickStatus(page, BACKUP)).toHaveAttribute('data-stick-status', 'error');
  await inspector(page).locator('[data-stick-problem-open]').click();
  await expect(problem(page)).toHaveAttribute('data-stick-problem', 'error');
  await expect(page.locator('[data-stick-problem-title]')).toHaveText('Sync to BACKUP 32GB failed');
  await page.evaluate(() => ((window as unknown as { __smokeFailSync: string | null }).__smokeFailSync = null));
  await page.getByRole('button', { name: 'Retry' }).click();
  await expect(problem(page)).toHaveCount(0);
  await expect(stickStatus(page, BACKUP)).toHaveAttribute('data-stick-status', 'idle');
  expect(await smoke.invokes('sync_usb_many')).toHaveLength(2);
});

test('All tracks on a stick: remove from stick and show in library from the row menu', async ({ smoke }) => {
  const { page } = smoke;
  await withSticks(page);
  await openStick(page, SANDISK);
  await view(page).locator('[data-stick-row="all"]').click();
  await expect(trackRows(page)).toHaveCount(2);
  await expect(view(page).locator('[data-stick-edit-hint]')).toContainText('It stays in your library');
  await trackRows(page).first().click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Remove from stick' }).click();
  expect(await smoke.invokes('stick_remove_tracks')).toEqual([{ path: SANDISK, trackIds: [7] }]);
  await expect(trackRows(page)).toHaveCount(1);
  await trackRows(page).first().click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Show in library' }).click();
  await expect(page.locator('[data-track-row]').first()).toBeVisible();
  await expect(page.locator('.ff-gheader__title')).toHaveText('All Tracks');
});

test('playlist on a stick: removing a track keeps the stick list apart from the library', async ({ smoke }) => {
  const { page } = smoke;
  await withSticks(page);
  await openStick(page, SANDISK);
  await listRow(page, 'Peak Time').click();
  expect(await trackTitles(page)).toEqual(['Smoke Track 01', 'Smoke Track 02', 'Smoke Track 03']);
  await trackRows(page).nth(1).click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Remove from this playlist on the stick' }).click();
  expect(await smoke.invokes('stick_set_playlist_tracks')).toEqual([{ path: SANDISK, playlistId: 1, trackIds: [1, 3] }]);
  expect(await trackTitles(page)).toEqual(['Smoke Track 01', 'Smoke Track 03']);
  await expect(page.locator('[data-stick-notice]')).toContainText('Your library did not change');
  const toggle = page.locator('[data-stick-playlist-header]').getByRole('switch', { name: 'Keep updated' });
  await expect(toggle).toHaveAttribute('aria-checked', 'false');
  await expect(sidebarPlaylist(page, 'Peak Time').locator('.ff-sbrow__count')).toHaveText('3');
  await inspector(page).getByRole('button', { name: 'Sync now' }).click();
  await page.getByRole('button', { name: 'Sync', exact: true }).last().click();
  const [call] = await smoke.invokes('sync_usb_many');
  expect(call.targets[0].selection.kept).toEqual([{ playlist_id: 1, track_ids: [1, 3] }]);
  await expect(stickStatus(page, SANDISK)).toHaveAttribute('data-stick-status', 'idle');
  await toggle.click();
  await expect(toggle).toHaveAttribute('aria-checked', 'true');
  expect(await trackTitles(page)).toEqual(['Smoke Track 01', 'Smoke Track 02', 'Smoke Track 03']);
  await expect(stickStatus(page, SANDISK)).toHaveAttribute('data-stick-status', 'pending');
});

test('playlist on a stick: drag a track to reorder it on the stick only', async ({ smoke }) => {
  const { page } = smoke;
  await withSticks(page);
  await openStick(page, SANDISK);
  await listRow(page, 'Peak Time').click();
  const from = (await trackRows(page).nth(2).boundingBox())!;
  const to = (await trackRows(page).nth(0).boundingBox())!;
  await page.mouse.move(from.x + 120, from.y + from.height / 2);
  await page.mouse.down();
  await page.mouse.move(from.x + 126, from.y, { steps: 4 });
  await page.mouse.move(to.x + 126, to.y + 3, { steps: 8 });
  await expect(page.locator('[data-stick-dropline]')).toBeVisible();
  await expect(page.locator('[data-stick-reorder-ghost]')).toHaveText('Move 1 track');
  await page.mouse.up();
  expect(await smoke.invokes('stick_set_playlist_tracks')).toEqual([{ path: SANDISK, playlistId: 1, trackIds: [3, 1, 2] }]);
  expect(await trackTitles(page)).toEqual(['Smoke Track 03', 'Smoke Track 01', 'Smoke Track 02']);
  await expect(page.locator('[data-stick-dropline]')).toHaveCount(0);
});

test('sync asks when tracks come off the stick', async ({ smoke }) => {
  const { page } = smoke;
  await withSticks(page);
  await openStick(page, SANDISK);
  await inspector(page).getByRole('button', { name: 'Sync now' }).click();
  const plan = page.locator('[data-stick-plan]');
  await expect(plan.locator('[data-plan-remove]')).toContainText('2 tracks come off the stick');
  await expect(plan.locator('[data-plan-foreign]')).toContainText('2 tracks put there by other apps');
  await shot(page, '06-removal-ask');
  expect(await smoke.invokes('sync_usb_many')).toEqual([]);
  await page.getByRole('button', { name: 'Sync', exact: true }).last().click();
  await expect(inspector(page).locator('[data-stick-status]')).toContainText('Safe to eject');
  expect(await smoke.invokes('sync_usb_many')).toHaveLength(1);
});

test("sync stops when it won't fit", async ({ smoke }) => {
  const { page } = smoke;
  await withSticks(page);
  await openStick(page, PANORAMA);
  await expect(inspector(page).locator('[data-stick-problem-open]')).toContainText('Too much for this stick');
  await inspector(page).getByRole('button', { name: 'Sync now' }).click();
  await expect(page.locator('[data-plan-wontfit]')).toContainText('Nothing was written');
  await expect(page.getByText("Won't fit on PANORAMA 8GB")).toBeVisible();
  await shot(page, '07-wont-fit');
  await page.getByRole('button', { name: 'OK' }).click();
  expect(await smoke.invokes('sync_usb_many')).toEqual([]);
});

test('stick in the side panel: a plain list with no info header, drop label on the board', async ({ smoke }) => {
  const { page } = smoke;
  await withSticks(page, 1280, 800);
  await stickRow(page, BACKUP).hover();
  await stickRow(page, BACKUP).locator('.ff-sbrow__action').click();
  const board = page.locator('[data-stick-board]');
  await expect(board.locator('[data-stick-list]')).toBeVisible();
  await expect(board.locator('[data-stick-row="all"]')).toContainText('All tracks');
  await expect(board.getByRole('button', { name: /Sync|Eject/ })).toHaveCount(0);
  await expect(board).not.toContainText('GB');
  await shot(page, '08-panel-stick-1280', page.locator('.ff-organize--expanded'));
  await drag(page, sidebarPlaylist(page, 'Peak Time'), () => board, async () => {
    await expect(board.locator('[data-drop-label]')).toHaveText('Put on BACKUP 32GB · Keep updated');
    await shot(page, '09-panel-drop-label');
  });
  expect(await smoke.invokes('stick_add_playlists')).toEqual([{ path: BACKUP, playlistIds: [1] }]);
});
