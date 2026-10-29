import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import type { Locator, Page } from '@playwright/test';
import { expect, sidebarPlaylist, test } from './app.ts';
import type { SmokeImport, SmokeWindow } from './tauri-mock.ts';

const SHOTS = resolve(import.meta.dirname, '../../.agent/file-drop');
mkdirSync(SHOTS, { recursive: true });

const FOLDER = '/Downloads/Fresh Finds';

function newTrack(id: number, file: string) {
  return {
    id,
    source_path: `${FOLDER}/${file}`,
    title: file.replace(/\.\w+$/, ''),
    artist: 'Drop Artist',
    album: 'Drop Album',
    genre: 'House',
    tempo: 12400,
    key: '8A',
    duration_secs: 300,
    bitrate: 320,
    file_size: 10_000_000,
    has_artwork: false,
    has_cues: false,
    has_analysis: false,
    analysis_pending: false,
  };
}

const TWO_NEW: SmokeImport = { tracks: [newTrack(101, 'Low Sun.mp3'), newTrack(102, 'Side Street.flac')], already: 0 };

async function nextImport(page: Page, next: SmokeImport) {
  await page.evaluate((n) => ((window as unknown as SmokeWindow).__smokeImport = n), next as never);
}

async function center(target: Locator) {
  const box = (await target.boundingBox())!;
  return { x: Math.round(box.x + box.width / 2), y: Math.round(box.y + box.height / 2) };
}

async function emit(page: Page, event: string, payload: unknown) {
  await page.evaluate(([e, p]) => (window as unknown as SmokeWindow).__smokeEmit(e as string, p), [event, payload] as const);
}

async function dragOver(page: Page, target: Locator, paths = [FOLDER]) {
  const position = await center(target);
  await emit(page, 'tauri://drag-enter', { paths, position });
  await emit(page, 'tauri://drag-over', { position });
}

async function dropOn(page: Page, target: Locator, paths = [FOLDER]) {
  await dragOver(page, target, paths);
  await emit(page, 'tauri://drag-drop', { paths, position: await center(target) });
}

const fill = (page: Page) => page.locator('.ff-sidebar__fill');
const table = (page: Page) => page.locator('[data-focus-region="tracks"]');
const content = (page: Page) => page.locator('[data-file-drop-content]');
const activity = (page: Page) => page.locator('[data-library-status]');
const lastImport = (page: Page) => page.locator('[data-last-import]');

async function openLastImport(page: Page) {
  const notNow = page.getByRole('button', { name: 'Not now' });
  if (await notNow.isVisible()) await notNow.click();
  await activity(page).click();
  await expect(lastImport(page)).toBeVisible();
  return lastImport(page);
}

async function savedPlaylists(smoke: { invokes(cmd: string): Promise<Record<string, any>[]> }) {
  const saves = await smoke.invokes('save_state');
  return (saves.at(-1)?.playlists ?? []) as { name: string; track_ids: number[]; parent_id: number }[];
}

test('hovering files highlights the target under the cursor, never a full-screen overlay', async ({ smoke }) => {
  const { page } = smoke;

  await dragOver(page, sidebarPlaylist(page, 'Warmup'));
  await expect(sidebarPlaylist(page, 'Warmup')).toHaveClass(/drag-over/);
  await expect(sidebarPlaylist(page, 'Peak Time')).not.toHaveClass(/drag-over/);
  await expect(content(page)).not.toHaveClass(/file-drop/);
  await page.screenshot({ path: `${SHOTS}/hover-playlist.png` });

  await dragOver(page, fill(page));
  await expect(fill(page)).toHaveClass(/drop-into/);
  await expect(sidebarPlaylist(page, 'Warmup')).not.toHaveClass(/drag-over/);
  await page.screenshot({ path: `${SHOTS}/hover-new-playlist.png` });

  await dragOver(page, table(page));
  await expect(content(page)).toHaveClass(/ff-browse__content--file-drop/);
  await expect(fill(page)).not.toHaveClass(/drop-into/);
  await page.screenshot({ path: `${SHOTS}/hover-table.png` });

  await expect(page.locator('.ff-file-drop-scrim, .ff-drop-overlay')).toHaveCount(0);

  await emit(page, 'tauri://drag-leave', null);
  await expect(content(page)).not.toHaveClass(/file-drop/);

  await dragOver(page, sidebarPlaylist(page, 'Warmup'));
  await page.keyboard.press('Escape');
  await expect(sidebarPlaylist(page, 'Warmup')).not.toHaveClass(/drag-over/);
});

test('a drop on a playlist imports the files and adds them to it', async ({ smoke }) => {
  const { page } = smoke;
  await nextImport(page, TWO_NEW);
  await dropOn(page, sidebarPlaylist(page, 'Warmup'));

  await expect(activity(page)).toHaveText('Added 2');
  await expect(await openLastImport(page)).toContainText('Added 2 tracks to ‘Warmup’.');
  expect(await smoke.invokes('plan_import')).toEqual([{ paths: [FOLDER] }]);
  const warmup = (await savedPlaylists(smoke)).find((p) => p.name === 'Warmup')!;
  expect(warmup.track_ids).toEqual([4, 101, 102]);
  await expect(sidebarPlaylist(page, 'Warmup')).not.toHaveClass(/drag-over/);
});

test('a drop on empty sidebar space creates a playlist named after the folder', async ({ smoke }) => {
  const { page } = smoke;
  await nextImport(page, TWO_NEW);
  await dropOn(page, fill(page));

  await expect(activity(page)).toHaveText('Added 2');
  await expect(await openLastImport(page)).toContainText('Created ‘Fresh Finds’ with 2 tracks.');
  const created = (await savedPlaylists(smoke)).find((p) => p.name === 'Fresh Finds')!;
  expect(created.track_ids).toEqual([101, 102]);
  expect(created.parent_id).toBe(0);
});

test('a drop on the track table imports into the library only', async ({ smoke }) => {
  const { page } = smoke;
  await nextImport(page, TWO_NEW);
  const before = (await smoke.invokes('save_state')).length;
  await dropOn(page, table(page));

  await expect(activity(page)).toHaveText('Added 2');
  expect(await smoke.invokes('commit_import')).toEqual([{ token: 1, skipDuplicates: true }]);
  expect(await smoke.invokes('save_state')).toHaveLength(before);
  await expect(content(page)).not.toHaveClass(/file-drop/);
});

test('re-importing files already in the library says so plainly', async ({ smoke }) => {
  const { page } = smoke;
  await nextImport(page, { tracks: [], already: 12 });
  await dropOn(page, table(page));
  await expect(activity(page)).toHaveText('All 12 files already in your library');

  await nextImport(page, { tracks: [], already: 0, skipped: { unsupported: 3, unsupported_exts: ['.m4p', '.txt'] } });
  await dropOn(page, table(page));
  await expect(activity(page)).toHaveText('3 files not imported');
  await expect(await openLastImport(page)).toContainText('3 files skipped: unsupported format (.m4p, .txt)');
  await page.screenshot({ path: `${SHOTS}/status-unsupported.png` });
});

test('a playlist drop of tracks already in the library adds them to the playlist', async ({ smoke }) => {
  const { page } = smoke;
  await nextImport(page, { tracks: [], already: 20 });
  await dropOn(page, sidebarPlaylist(page, 'Warmup'), ['/Music/smoke']);

  await expect(activity(page)).toHaveText('All 20 files already in your library');
  await expect(await openLastImport(page)).toContainText('Added 19 tracks to ‘Warmup’ (already in library).');
  const warmup = (await savedPlaylists(smoke)).find((p) => p.name === 'Warmup')!;
  expect(warmup.track_ids).toHaveLength(20);
  await page.screenshot({ path: `${SHOTS}/status-already-in-library.png` });
});

test('the duplicates question comes first and Skip duplicates adds the library tracks to the playlist', async ({ smoke }) => {
  const { page } = smoke;
  await page.evaluate(
    (rows) => ((window as unknown as SmokeWindow).__smokeDuplicates = rows),
    [
      { new_path: `${FOLDER}/copy-a.mp3`, file_name: 'copy-a.mp3', existing_id: 7, title: 'Smoke Track 07', artist: 'Four Tet', duration_secs: 222, reason: 'same_tags' },
      { new_path: `${FOLDER}/copy-b.mp3`, file_name: 'copy-b.mp3', existing_id: 8, title: 'Smoke Track 08', artist: 'Ben Böhmer', duration_secs: 229, reason: 'same_tags' },
    ],
  );
  await nextImport(page, TWO_NEW);
  const before = (await smoke.invokes('save_state')).length;
  await dropOn(page, sidebarPlaylist(page, 'Peak Time'));

  const dialog = page.locator('.ff-dialog').filter({ has: page.locator('[data-duplicates-dialog]') });
  await expect(dialog).toBeVisible();
  expect(await smoke.invokes('save_state')).toHaveLength(before);
  await dialog.getByRole('button', { name: 'Skip duplicates' }).click();

  await expect(activity(page)).toHaveText('Added 2 · 2 duplicates skipped');
  await expect(await openLastImport(page)).toContainText('Added 4 tracks to ‘Peak Time’ (2 new, 2 already in library).');
  await page.keyboard.press('Escape');
  const peak = (await savedPlaylists(smoke)).find((p) => p.name === 'Peak Time')!;
  expect(peak.track_ids).toEqual([1, 2, 3, 101, 102, 7, 8]);
});
