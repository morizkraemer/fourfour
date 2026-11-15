import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import type { Page } from '@playwright/test';
import { TRACK_COUNT } from './fixture.ts';
import { expect, rows, test } from './app.ts';

const SHOTS = resolve(import.meta.dirname, '../../.agent/backup');
mkdirSync(SHOTS, { recursive: true });

type MockWindow = { __smokePickPath: string; __smokeHoldExport: boolean; __smokeReleaseExport: (() => void) | null };

async function openBackupPage(page: Page) {
  await page.keyboard.press('Control+,');
  await page.locator('.ff-settings-sheet__nav-item').filter({ hasText: 'Backup' }).click();
  await expect(page.locator('.ff-settings-sheet__heading')).toHaveText('Backup');
}

const saveCard = (page: Page) => page.locator('[data-backup-save]');
const restoreCard = (page: Page) => page.locator('[data-backup-restore]');
const restoreDialog = (page: Page) => page.locator('.ff-dialog').filter({ has: page.locator('[data-restore-dialog]') });

test('Settings → Backup saves the library with audio and shows progress', async ({ smoke }) => {
  const { page } = smoke;
  await page.evaluate(() => localStorage.setItem('fourfour.quantize.v1', '0'));
  await openBackupPage(page);
  await expect(saveCard(page)).toContainText('Library and analysis: about 38.0 MB. With audio: about 61.0 GB');
  await expect(saveCard(page)).toContainText('No backup yet.');
  await page.screenshot({ path: `${SHOTS}/settings-backup.png` });

  await saveCard(page).getByRole('button', { name: 'Also audio files' }).click();
  await page.evaluate(() => ((window as unknown as MockWindow).__smokeHoldExport = true));
  await saveCard(page).getByRole('button', { name: 'Save backup…' }).click();
  await expect(saveCard(page)).toContainText('27.4 GB of 61.0 GB · 44%');
  await expect(saveCard(page).locator('.ff-progress-bar')).toBeVisible();
  await page.screenshot({ path: `${SHOTS}/settings-backup-progress.png` });

  await page.evaluate(() => (window as unknown as MockWindow).__smokeReleaseExport?.());
  await expect(saveCard(page)).toContainText(/Last backup today: library-backup-\d{4}-\d{2}-\d{2}\.tar\.gz \(38\.0 MB\)\./);
  const [call] = await smoke.invokes('export_backup');
  expect(call.path).toMatch(/^\/Backups\/library-backup-\d{4}-\d{2}-\d{2}\.tar\.gz$/);
  expect(call.includeAudio).toBe(true);
  expect(JSON.parse(call.uiSettings)).toMatchObject({ 'fourfour.quantize.v1': '0' });
});

test('restore asks first, then replaces the library and settings', async ({ smoke }) => {
  const { page } = smoke;
  await page.evaluate(() => ((window as unknown as MockWindow).__smokePickPath = '/Backups/library-backup-2026-10-17.tar.gz'));
  await openBackupPage(page);

  await restoreCard(page).getByRole('button', { name: 'Choose file…' }).click();
  await expect(restoreDialog(page).locator('.ff-dialog__title')).toHaveText('Restore this backup?');
  await expect(restoreDialog(page)).toContainText('library-backup-2026-10-17.tar.gz · 176 tracks · made 12 days ago');
  await expect(restoreDialog(page)).toContainText(`Replaces your current ${TRACK_COUNT} tracks, playlists and settings; they are saved as a backup first`);
  await expect(restoreDialog(page)).toContainText('No audio inside: tracks point to the same files as before');
  await page.screenshot({ path: `${SHOTS}/restore-confirm.png` });

  await restoreDialog(page).getByRole('button', { name: 'Cancel' }).click();
  await expect(restoreDialog(page)).toHaveCount(0);
  expect(await smoke.invokes('restore_backup')).toEqual([]);

  await restoreCard(page).getByRole('button', { name: 'Choose file…' }).click();
  await Promise.all([page.waitForEvent('load'), restoreDialog(page).getByRole('button', { name: 'Restore' }).click()]);
  await rows(page).first().waitFor();
  expect(await page.evaluate(() => JSON.parse(sessionStorage.getItem('smoke.restore') ?? 'null'))).toEqual({
    path: '/Backups/library-backup-2026-10-17.tar.gz',
    audioFolder: null,
  });
  expect(await page.evaluate(() => localStorage.getItem('fourfour.quantize.v1'))).toBe('1');
});

test('a file that is not a backup is refused before anything is asked', async ({ smoke }) => {
  const { page } = smoke;
  await page.evaluate(() => ((window as unknown as MockWindow).__smokePickPath = '/Downloads/notes.txt'));
  await openBackupPage(page);
  await restoreCard(page).getByRole('button', { name: 'Choose file…' }).click();
  await expect(restoreCard(page)).toContainText("That file isn't a fourfour library backup. Choose another file.");
  await expect(page.locator('[data-restore-dialog]')).toHaveCount(0);
});
