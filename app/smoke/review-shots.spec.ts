import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import type { Page } from '@playwright/test';
import { expect, rows, test } from './app.ts';

const SHOTS = resolve(import.meta.dirname, '../../.agent/review');
mkdirSync(SHOTS, { recursive: true });

const WINDOW = { width: 1440, height: 900 };
const shot = (page: Page, name: string) => page.screenshot({ path: `${SHOTS}/${name}.png`, animations: 'disabled' });
const menuItems = (page: Page) => page.locator('.ff-context-menu .ff-menu__item');
const statusButton = (page: Page) => page.locator('[data-library-status]');

function dupMember(id: number, artist: string, title: string, format: string, lossless: boolean, bitrate: number, secs: number, playlists: number, cues: number) {
  return {
    id,
    artist,
    title,
    path: `/Music/${format === 'mp3' ? 'Downloads' : 'Library'}/${artist} - ${title}.${format}`,
    format,
    lossless,
    bitrate,
    sample_rate: 44100,
    duration_secs: secs,
    file_size: Math.round((secs * bitrate * 1000) / 8),
    playlists,
    cues,
    date_added: '2026-10-04',
    file_present: true,
  };
}

async function setWindowState(page: Page, state: Record<string, unknown>) {
  await page.evaluate((values) => Object.assign(window, values), state);
}

async function withMissing(page: Page, missing: number[]) {
  await page.addInitScript((m) => ((window as unknown as Record<string, unknown>).__smokeMissing = m), missing);
  await page.reload();
  await rows(page).first().waitFor();
}

test.use({ viewport: WINDOW });

test('review: playlist export entry points', async ({ smoke }) => {
  const { page } = smoke;
  await page.evaluate(() => localStorage.setItem('smoke.reviewFolder', '1'));
  await page.reload();
  await rows(page).first().waitFor();

  await page.locator('[data-drop-track-playlist-id]').filter({ hasText: 'Peak Time' }).click({ button: 'right' });
  await expect(menuItems(page).filter({ hasText: 'Export playlist…' })).toBeVisible();
  await shot(page, 'export-1-playlist-menu');

  await menuItems(page).filter({ hasText: 'Export playlist…' }).hover();
  await expect(page.getByRole('menuitem', { name: 'Rekordbox XML' })).toBeVisible();
  await shot(page, 'export-2-format-submenu');
  await page.keyboard.press('Escape');
  await page.keyboard.press('Escape');

  await page.locator('.ff-sbrow__label', { hasText: 'Festival sets' }).click({ button: 'right' });
  await expect(menuItems(page).filter({ hasText: 'Export folder…' })).toBeVisible();
  await shot(page, 'export-3-folder-menu');
});

test('review: moved-file reconnect', async ({ smoke }) => {
  const { page } = smoke;
  await withMissing(page, [3, 5, 7, 9, 11]);
  await setWindowState(page, {
    __smokeReconnectPlan: {
      reconnect: [
        { track_id: 3, source_path: '/Volumes/Archive/Music/Smoke Track 03.mp3' },
        { track_id: 5, source_path: '/Volumes/Archive/Music/Smoke Track 05.mp3' },
      ],
      needs_choice: [
        { track_id: 7, reason: 'multiple_candidates', candidates: ['/Volumes/Archive/Music/House/Smoke Track 07.mp3', '/Volumes/Archive/Music/Old/Smoke Track 07.mp3'] },
        { track_id: 9, reason: 'copy', candidates: ['/Volumes/Archive/Music/Copies/Smoke Track 09.mp3'] },
      ],
      still_missing: [11],
    },
    __smokeRemapPreview: { total: 5, found: 4, still_missing: 1 },
    __smokePickPath: '/Volumes/Archive/Music',
  });

  await statusButton(page).click();
  await menuItems(page).filter({ hasText: 'Library check results…' }).click();
  await expect(page.locator('[data-reconnect-panel]')).toBeVisible();
  await shot(page, 'reconnect-1-check-dialog');

  await page.getByRole('button', { name: 'More' }).click();
  await expect(menuItems(page)).toHaveText(['Relocate missing…', 'Relocate folder…']);
  await shot(page, 'reconnect-1b-more-menu');
  await page.keyboard.press('Escape');
  await expect(menuItems(page)).toHaveCount(0);
  await expect(page.locator('[data-health-dialog]')).toBeVisible();

  await page.getByRole('button', { name: 'Find moved files…' }).click();
  await expect(page.locator('[data-reconnect-result]')).toContainText('2 tracks reconnected, 2 need your choice, 1 still missing.');
  await shot(page, 'reconnect-2-find-moved-result');

  await page.getByRole('button', { name: 'More' }).click();
  await menuItems(page).filter({ hasText: 'Relocate folder…' }).click();
  await expect(page.locator('[data-relocate-folder]')).toBeVisible();
  await page.getByRole('button', { name: 'Choose…' }).click();
  await expect(page.locator('[data-relocate-preview]')).toContainText('4 found');
  await shot(page, 'reconnect-3-relocate-form');

  await page.getByRole('button', { name: 'Relocate', exact: true }).click();
  await expect(page.locator('[data-relocate-last]')).toBeVisible();
  await shot(page, 'reconnect-4-relocated-undo');
});

test('review: duplicate cleanup', async ({ smoke }) => {
  const { page } = smoke;
  const groups = [
    {
      key: 1,
      confidence: 'exact_audio',
      keeper_id: 1,
      reason: 'lossless beats lossy',
      members: [
        dupMember(1, 'Ode & Ash', 'Glass Harbor', 'aiff', true, 1411, 425, 3, 4),
        dupMember(2, 'Ode & Ash', 'Glass Harbor', 'mp3', false, 320, 425, 0, 0),
      ],
    },
    {
      key: 2,
      confidence: 'exact_audio',
      keeper_id: 3,
      reason: 'in the most playlists',
      members: [
        dupMember(3, 'Mira Sol', 'Silent Hours', 'wav', true, 1411, 402, 2, 1),
        dupMember(4, 'Mira Sol', 'Silent Hours', 'wav', true, 1411, 402, 0, 0),
      ],
    },
    {
      key: 3,
      confidence: 'metadata_only',
      keeper_id: 5,
      reason: 'higher bitrate',
      members: [
        dupMember(5, 'Hale', 'Copper Lines', 'flac', true, 980, 380, 1, 2),
        dupMember(6, 'Hale', 'Copper Lines (Edit)', 'mp3', false, 256, 381, 0, 0),
      ],
    },
  ];
  const archive = [
    {
      merge_id: 1,
      merged_at: '2026-11-20T18:30:00Z',
      confidence: 'exact_audio',
      keeper_id: 1,
      keeper_label: 'Ode & Ash - Glass Harbor',
      tracks: [{ id: 2, artist: 'Ode & Ash', title: 'Glass Harbor', path: '/Music/Downloads/Ode & Ash - Glass Harbor.mp3', format: 'mp3', lossless: false, bitrate: 320, file_present: true }],
    },
  ];
  await setWindowState(page, { __smokeDupGroups: groups, __smokeDupArchive: archive });

  await statusButton(page).click();
  await expect(menuItems(page).filter({ hasText: 'Find duplicates…' })).toBeVisible();
  await shot(page, 'duplicates-1-activity-menu');

  await menuItems(page).filter({ hasText: 'Find duplicates…' }).click();
  await expect(page.locator('[data-duplicates-view]')).toBeVisible();
  await expect(page.locator('[data-duplicate-group]')).toHaveCount(3);
  await expect(page.getByRole('button', { name: 'Merge 2 exact matches' })).toBeVisible();
  await page.mouse.move(0, 0);
  await shot(page, 'duplicates-2-groups');

  await page.locator('[data-duplicate-member="2"]').hover();
  await shot(page, 'duplicates-2b-group-hover');

  await page.locator('[data-duplicate-member="2"]').click();
  await expect(page.locator('[data-duplicate-member="2"]')).toHaveAttribute('aria-checked', 'true');
  await expect(page.locator('[data-duplicate-group="1"] [data-duplicate-reason]')).toHaveText('Keeper: your pick');
  await page.mouse.move(0, 0);
  await page.locator('[data-duplicate-group="2"]').getByRole('button', { name: 'Skip' }).focus();
  await page.keyboard.press('Tab');
  await page.keyboard.press('Shift+Tab');
  await shot(page, 'duplicates-2c-group-focus');

  await page.getByText('Archive (1)', { exact: true }).click();
  await expect(page.locator('[data-archived-merge]')).toHaveCount(1);
  await shot(page, 'duplicates-3-archive');
});
