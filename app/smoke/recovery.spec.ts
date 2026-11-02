import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { test, expect, type Page } from '@playwright/test';
import { fixtureAnalysis, fixtureState } from './fixture.ts';
import { installTauriMock, type SmokeWindow } from './tauri-mock.ts';

const SHOTS = process.env.FF_RECOVERY_SHOTS ?? resolve(import.meta.dirname, '../../.agent/recovery');
mkdirSync(SHOTS, { recursive: true });

const LIBRARY_DB = '/smoke/library/library.db';
type WireError = { kind: string; message: string; path?: string };
const startupError = (kind: string, reason: string): WireError => ({
  kind,
  message: `Couldn't open the library at ${LIBRARY_DB}: ${reason}`,
  path: LIBRARY_DB,
});
const NEWER = startupError('library_newer_version', 'The library was saved by a newer version of the app (schema version 99, this app knows up to 7); update the app to open it');
const DAMAGED = startupError('library_damaged', 'file is not a database');
const LOCKED = startupError('library_locked', "The library's schema version is unreadable; it was not opened so nothing in it is changed: database is locked");

type MockWindow = SmokeWindow & { __smokePickPath: string };

async function openFailedApp(page: Page, error: WireError) {
  await page.exposeFunction('__smokeAnalysis', (trackId: number) => fixtureAnalysis(trackId));
  await page.addInitScript(installTauriMock, fixtureState());
  await page.goto('/');
  await page.evaluate((message) => localStorage.setItem('smoke.loadError', JSON.stringify(message)), error);
  await page.reload();
  await page.locator('[data-library-recovery]').waitFor();
}

const panel = (page: Page) => page.locator('[data-library-recovery]');
const invokes = (page: Page, cmd: string) =>
  page.evaluate((name) => (window as unknown as MockWindow).__smokeInvokes.filter((i) => i.cmd === name).map((i) => i.args as Record<string, unknown>), cmd);
const rows = (page: Page) => page.locator('[data-track-row]');

test('a library that is too new says so and offers the three ways out', async ({ page }) => {
  await openFailedApp(page, NEWER);
  await expect(panel(page)).toHaveAttribute('data-failure', 'newer');
  await expect(panel(page)).toContainText('This library was made by a newer version of fourfour');
  await expect(panel(page)).toContainText('The library file has not been changed.');
  await expect(panel(page).getByRole('button')).toHaveText(['Open another library', 'Restore a backup', 'Start a new library']);
  await expect(page.getByText('No tracks loaded')).toHaveCount(0);
  await page.screenshot({ path: `${SHOTS}/newer.png` });
});

test('a damaged library says so and offers the three ways out', async ({ page }) => {
  await openFailedApp(page, DAMAGED);
  await expect(panel(page)).toHaveAttribute('data-failure', 'damaged');
  await expect(panel(page)).toContainText("Couldn't open your library");
  await expect(panel(page)).toContainText('The library file is damaged. It has not been changed.');
  await expect(panel(page).getByRole('button')).toHaveText(['Open another library', 'Restore a backup', 'Start a new library']);
  await page.screenshot({ path: `${SHOTS}/damaged.png` });
});

test('a locked library offers Try again, which opens it once the lock is gone', async ({ page }) => {
  await openFailedApp(page, LOCKED);
  await expect(panel(page)).toHaveAttribute('data-failure', 'locked');
  await expect(panel(page)).toContainText('Your library is in use');
  await expect(panel(page).getByRole('button')).toHaveText(['Try again', 'Open another library', 'Restore a backup', 'Start a new library']);
  await page.screenshot({ path: `${SHOTS}/locked.png` });

  await panel(page).getByRole('button', { name: 'Try again' }).click();
  await expect(panel(page).locator('[data-recovery-error]')).toContainText("Still can't open the library. Close the other program and try again.");
  await page.screenshot({ path: `${SHOTS}/locked-still.png` });

  await page.evaluate(() => ((window as unknown as MockWindow).__smokeLoadError = null));
  await panel(page).getByRole('button', { name: 'Try again' }).click();
  await rows(page).first().waitFor();
  await expect(panel(page)).toHaveCount(0);
  expect(await invokes(page, 'recover_retry_library')).toHaveLength(2);
});

test('Open another library registers the chosen folder and loads it without a restart', async ({ page }) => {
  await openFailedApp(page, DAMAGED);
  await page.evaluate(() => ((window as unknown as MockWindow).__smokePickPath = '/Music/Other library'));
  await panel(page).getByRole('button', { name: 'Open another library' }).click();
  await rows(page).first().waitFor();
  await expect(panel(page)).toHaveCount(0);
  expect(await invokes(page, 'recover_open_library')).toEqual([{ folderPath: '/Music/Other library' }]);
  await expect(page.locator('.ff-statusbar-module')).toContainText('still at /smoke/library/library.db');
});

test('Start a new library needs no questions and loads the empty library', async ({ page }) => {
  await openFailedApp(page, DAMAGED);
  await panel(page).getByRole('button', { name: 'Start a new library' }).click();
  await rows(page).first().waitFor();
  await expect(panel(page)).toHaveCount(0);
  expect(await invokes(page, 'recover_new_library')).toHaveLength(1);
});

test('Restore a backup asks first, restores into a new library and reloads the window', async ({ page }) => {
  await openFailedApp(page, DAMAGED);
  await page.evaluate(() => ((window as unknown as MockWindow).__smokePickPath = '/Backups/library-backup-2026-10-17.tar.gz'));
  await panel(page).getByRole('button', { name: 'Restore a backup' }).click();
  const dialog = page.locator('.ff-dialog').filter({ has: page.locator('[data-restore-dialog]') });
  await expect(dialog).toContainText('library-backup-2026-10-17.tar.gz · 176 tracks');
  await expect(dialog).toContainText("The backup becomes your library. The library file that wouldn't open is left as it is");
  await expect(dialog).not.toContainText('Replaces your current');
  await page.screenshot({ path: `${SHOTS}/restore-confirm.png` });

  await dialog.getByRole('button', { name: 'Cancel' }).click();
  expect(await invokes(page, 'recover_restore_backup')).toEqual([]);

  await panel(page).getByRole('button', { name: 'Restore a backup' }).click();
  await Promise.all([page.waitForEvent('load'), dialog.getByRole('button', { name: 'Restore' }).click()]);
  await rows(page).first().waitFor();
  expect(await page.evaluate(() => JSON.parse(sessionStorage.getItem('smoke.restore') ?? 'null'))).toEqual({
    path: '/Backups/library-backup-2026-10-17.tar.gz',
    audioFolder: null,
  });
  expect(await page.evaluate(() => localStorage.getItem('fourfour.quantize.v1'))).toBe('1');
  await expect(panel(page)).toHaveCount(0);
});

test('an action that fails keeps the panel and says why in plain words', async ({ page }) => {
  await openFailedApp(page, DAMAGED);
  await page.evaluate(() => {
    const w = window as unknown as { __TAURI_INTERNALS__: { invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> } };
    const real = w.__TAURI_INTERNALS__.invoke;
    w.__TAURI_INTERNALS__.invoke = (cmd, args) =>
      cmd === 'recover_open_library'
        ? Promise.reject({ kind: 'library_damaged', message: "Couldn't open the library at /Music/Other/library.db: file is not a database", path: '/Music/Other/library.db' })
        : real(cmd, args);
  });
  await panel(page).getByRole('button', { name: 'Open another library' }).click();
  await expect(panel(page).locator('[data-recovery-error]')).toHaveText("fourfour can't read that library file. It has not been changed.");
  await expect(panel(page)).toHaveAttribute('data-failure', 'damaged');
  await page.screenshot({ path: `${SHOTS}/open-failed.png` });
});
