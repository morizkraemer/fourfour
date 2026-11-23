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

function dupMember(id: number, artist: string, title: string, path: string, bitrate: number, secs: number, playlists: number, cues: number) {
  const format = path.slice(path.lastIndexOf('.') + 1).toUpperCase();
  return {
    id,
    artist,
    title,
    path,
    format,
    lossless: format !== 'MP3',
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

  await expect(page.locator('[data-reconnect-options] .ff-btn')).toHaveText(['Find moved files…', 'Relocate folder…', 'Relocate missing…']);
  await expect(page.locator('[data-reconnect-options] .ff-btn--primary')).toHaveText('Find moved files…');

  await page.getByRole('button', { name: 'Find moved files…' }).click();
  await expect(page.locator('[data-reconnect-result]')).toContainText('2 tracks reconnected, 2 need your choice, 1 still missing.');
  await shot(page, 'reconnect-2-find-moved-result');

  await page.getByRole('button', { name: 'Relocate folder…' }).click();
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
      reason: 'Lossless (AIFF) over lossy (MP3)',
      members: [
        dupMember(1, 'Ode & Ash', 'Glass Harbor', '/Users/dj/Music/Library/Ode & Ash/Glass Harbor.aiff', 1411, 425, 3, 4),
        dupMember(2, 'Ode & Ash', 'Glass Harbor', '/Users/dj/Downloads/ode_ash-glass_harbor-320.mp3', 320, 425, 0, 0),
      ],
    },
    {
      key: 2,
      confidence: 'exact_audio',
      keeper_id: 3,
      reason: 'In more playlists (2 vs 0)',
      members: [
        dupMember(3, 'Mira Sol', 'Silent Hours', '/Users/dj/Music/Library/Mira Sol/Silent Hours.wav', 1411, 402, 2, 1),
        dupMember(4, 'Mira Sol', 'Silent Hours', '/Users/dj/Music/Library/Mira Sol/Silent Hours (1).wav', 1411, 402, 0, 0),
      ],
    },
    {
      key: 3,
      confidence: 'metadata_only',
      keeper_id: 5,
      reason: 'Lossless (FLAC) over lossy (MP3)',
      members: [
        dupMember(5, 'Hale', 'Copper Lines', '/Users/dj/Music/Library/Hale/Copper Lines.flac', 980, 380, 1, 2),
        dupMember(6, 'Hale', 'Copper Lines', '/Users/dj/Music/Promos/2024/Hale - Copper Lines.mp3', 256, 381, 0, 3),
        dupMember(7, 'Hale', 'Copper Lines', '/Volumes/Gigs SSD/Sets/2025 Summer/Copper Lines.mp3', 320, 380, 1, 0),
      ],
    },
  ];
  const archive = [
    {
      merge_id: 1,
      merged_at: '2026-11-20T18:30:00Z',
      confidence: 'exact_audio',
      keeper_id: 11,
      keeper_label: 'Tomas Reyne - Low Tide Ritual',
      tracks: [{ id: 12, artist: 'Tomas Reyne', title: 'Low Tide Ritual', path: '/Users/dj/Downloads/Low Tide Ritual (Club).mp3', format: 'MP3', lossless: false, bitrate: 320, file_present: true }],
    },
    {
      merge_id: 2,
      merged_at: '2026-11-18T10:05:00Z',
      confidence: 'exact_audio',
      keeper_id: 13,
      keeper_label: 'Ivo Brandt - Paper Moons',
      tracks: [
        { id: 14, artist: 'Ivo Brandt', title: 'Paper Moons', path: '/Users/dj/Music/Old/Paper Moons.wav', format: 'WAV', lossless: true, bitrate: 1411, file_present: true },
        { id: 15, artist: 'Ivo Brandt', title: 'Paper Moons', path: '/Volumes/Gigs SSD/Paper Moons.wav', format: 'WAV', lossless: true, bitrate: 1411, file_present: true },
      ],
    },
  ];
  await setWindowState(page, { __smokeDupGroups: groups, __smokeDupArchive: archive });
  const pick = (key: number) => page.locator(`[data-duplicate-pick="${key}"]`);
  const keeper = (id: number) => page.locator(`[data-duplicate-member="${id}"] input`);
  const open = async () => {
    await statusButton(page).click();
    await menuItems(page).filter({ hasText: 'Find duplicates…' }).click();
    await expect(page.locator('[data-duplicates-view]')).toBeVisible();
  };

  await statusButton(page).click();
  await expect(menuItems(page).filter({ hasText: 'Find duplicates…' })).toBeVisible();
  await shot(page, 'duplicates-1-activity-menu');

  await menuItems(page).filter({ hasText: 'Find duplicates…' }).click();
  await expect(page.locator('[data-duplicates-view]')).toBeVisible();
  await expect(page.locator('[data-duplicate-group]')).toHaveCount(3);
  await expect(pick(1)).toBeChecked();
  await expect(pick(2)).toBeChecked();
  await expect(pick(3)).not.toBeChecked();
  await expect(keeper(1)).toBeChecked();
  await expect(page.locator('[data-duplicate-member="1"]')).toContainText('Keep AIFF · in 3 playlists · 4 cues');
  await expect(page.locator('[data-duplicate-member="2"]')).toContainText('Merge MP3 320 kbps · no playlists or cues');
  await expect(page.locator('[data-duplicate-member="4"]')).toContainText('Silent Hours (1).wav');
  await page.mouse.move(0, 0);
  await shot(page, 'duplicates-2-groups');

  await page.locator('[data-duplicate-member="2"]').hover();
  await shot(page, 'duplicates-2b-group-hover');

  await page.locator('[data-duplicate-member="2"]').click();
  await expect(keeper(2)).toBeChecked();
  await expect(page.locator('[data-duplicate-group="1"] [data-duplicate-reason]')).toHaveText('Identical audio · Your pick');
  await page.keyboard.press('ArrowUp');
  await expect(keeper(1)).toBeChecked();
  await expect(page.locator('[data-duplicate-group="1"] [data-duplicate-reason]')).toHaveText('Identical audio · Suggested: Lossless (AIFF) over lossy (MP3)');
  await page.mouse.move(0, 0);
  await shot(page, 'duplicates-2c-group-focus');

  await pick(3).focus();
  await page.keyboard.press('Space');
  await expect(pick(3)).toBeChecked();
  await expect(page.getByRole('button', { name: 'Merge 3 songs' })).toBeVisible();
  await page.keyboard.press('Space');
  await expect(page.getByRole('button', { name: 'Merge 2 songs' })).toBeVisible();
  await page.getByRole('button', { name: 'Merge 2 songs' }).focus();

  await page.setViewportSize({ width: 1100, height: 700 });
  await page.mouse.move(0, 0);
  await shot(page, 'duplicates-2d-narrow');
  await page.setViewportSize(WINDOW);

  await page.locator('[data-duplicate-group="2"] .ff-dedupe__title').click();
  await expect(pick(2)).not.toBeChecked();
  await page.getByRole('button', { name: 'Merge 1 song' }).click();
  await expect(page.locator('[data-duplicates-message]')).toHaveText('Merged 1 copy into 1 keeper. No file was deleted.');
  await expect(page.locator('[data-duplicate-group]')).toHaveCount(2);
  expect(await smoke.invokes('apply_duplicate_merges')).toEqual([{ requests: [{ keeper_id: 1, remove_ids: [2], require_identical: true }] }]);
  await page.mouse.move(0, 0);
  await shot(page, 'duplicates-2e-after-merge');

  await page.getByText('Archive (3)', { exact: true }).click();
  await expect(page.locator('[data-archived-merge]')).toHaveCount(3);
  await page.mouse.move(0, 0);
  await shot(page, 'duplicates-3-archive');

  await page.locator('[data-archived-merge="3"]').getByRole('button', { name: 'Restore' }).click();
  await expect(page.locator('[data-duplicates-message]')).toHaveText('Restored 1 track with playlists and cues.');
  await expect(page.locator('[data-archived-merge]')).toHaveCount(2);

  await page.getByRole('button', { name: 'Done' }).click();
  await setWindowState(page, { __smokeDupGroups: groups.slice(0, 1), __smokeDupArchive: [] });
  await open();
  await expect(page.getByRole('button', { name: 'Merge 1 song' })).toBeVisible();
  await page.mouse.move(0, 0);
  await shot(page, 'duplicates-4-one-song');
  await page.getByText('Archive', { exact: true }).click();
  await expect(page.locator('[data-archive-empty]')).toBeVisible();
  await shot(page, 'duplicates-5-archive-empty');

  await page.getByRole('button', { name: 'Done' }).click();
  await setWindowState(page, { __smokeDupGroups: [] });
  await open();
  await expect(page.locator('[data-duplicates-empty]')).toBeVisible();
  await shot(page, 'duplicates-6-none');
});
