import { test, expect, type Page } from '@playwright/test';
import { fixtureAnalysis, fixtureState } from './fixture.ts';
import { installTauriMock, type SmokeWindow } from './tauri-mock.ts';

async function openApp(page: Page, state: ReturnType<typeof fixtureState>) {
  await page.exposeFunction('__smokeAnalysis', (trackId: number) => fixtureAnalysis(trackId));
  await page.addInitScript(installTauriMock, state);
  await page.addInitScript(() => ((window as unknown as SmokeWindow).__smokeHoldLoad = true));
  await page.goto('/');
}

const releaseLoad = (page: Page) => page.evaluate(() => (window as unknown as SmokeWindow).__smokeReleaseLoad?.());

test('skeleton rows show instead of the empty state until the library loads', async ({ page }) => {
  await openApp(page, fixtureState());
  await expect(page.locator('[data-table-skeleton]')).toBeVisible();
  await expect(page.getByText('No tracks loaded')).toHaveCount(0);
  await releaseLoad(page);
  await page.locator('[data-track-row]').first().waitFor();
  await expect(page.locator('[data-table-skeleton]')).toHaveCount(0);
});

test('a library that really has no tracks shows the empty state after loading', async ({ page }) => {
  await openApp(page, { ...fixtureState(), tracks: [], playlists: [] });
  await expect(page.getByText('No tracks loaded')).toHaveCount(0);
  await releaseLoad(page);
  await expect(page.getByText('No tracks loaded')).toBeVisible();
  await expect(page.locator('[data-table-skeleton]')).toHaveCount(0);
});

const bootSnapshotIds = (page: Page) =>
  page.evaluate(() => {
    const raw = localStorage.getItem('fourfour.bootSnapshot.v1');
    return raw ? (JSON.parse(raw).rows as { cells: { id: number } }[]).map((r) => r.cells.id) : null;
  });

test('the last open list paints from the boot snapshot, then the real load replaces it', async ({ context }) => {
  const first = await context.newPage();
  await openApp(first, fixtureState());
  await releaseLoad(first);
  await first.locator('[data-track-row]').first().waitFor();
  await expect.poll(() => bootSnapshotIds(first), { timeout: 10_000 }).not.toBeNull();
  const snapshotIds = (await bootSnapshotIds(first))!;
  expect(snapshotIds.length).toBeGreaterThan(5);
  expect(snapshotIds.slice(0, 3)).toEqual([1, 2, 3]);
  await first.close();

  const base = fixtureState();
  const changed = {
    ...base,
    tracks: base.tracks.filter((t) => t.id !== 1).map((t) => (t.id === 2 ? { ...t, title: 'Renamed Outside' } : t)),
  };
  const second = await context.newPage();
  await openApp(second, changed);
  await expect(second.locator('[data-boot-row]').first()).toBeVisible();
  await expect(second.locator('[data-table-skeleton]')).toHaveCount(0);
  await expect(second.locator('[data-table-boot]')).toHaveAttribute('inert', '');
  await expect(second.locator('[data-boot-row]').first()).toContainText('Smoke Track 01');
  await releaseLoad(second);
  await second.locator('[data-track-row]').first().waitFor();
  await expect(second.locator('[data-boot-row]')).toHaveCount(0);
  await expect(second.locator('[data-track-row]').first()).toContainText('Renamed Outside');
  await expect(second.getByText('Smoke Track 01', { exact: true })).toHaveCount(0);
  await expect.poll(() => bootSnapshotIds(second), { timeout: 10_000 }).not.toContain(1);
  expect(await bootSnapshotIds(second)).toEqual(expect.arrayContaining([2, 3]));
});

test('rows on screen fetch their previews after the list loads', async ({ page }) => {
  const base = fixtureState();
  const bands = Buffer.from(new Uint8Array(384).map((_, i) => 20 + (i % 90))).toString('base64');
  await openApp(page, { ...base, tracks: base.tracks.map((t) => ({ ...t, waveform_bands: bands })) });
  await releaseLoad(page);
  await page.locator('[data-track-row]').first().waitFor();
  const asked = async () =>
    (await page.evaluate(() => (window as unknown as SmokeWindow).__smokeInvokes.filter((i) => i.cmd === 'get_row_previews')))
      .flatMap((i) => (i.args as { trackIds: number[] }).trackIds);
  await expect.poll(asked).toContain(1);
  const ids = await asked();
  expect(new Set(ids).size).toBe(ids.length);
  const shown = await page.locator('[data-track-row]').evaluateAll((rows) => rows.map((r) => Number((r as HTMLElement).dataset.rowIndex) + 1));
  expect(ids).toEqual(expect.arrayContaining(shown));
});

test('a snapshot from another library is not shown', async ({ page }) => {
  await page.addInitScript(() =>
    localStorage.setItem(
      'fourfour.bootSnapshot.v1',
      JSON.stringify({ libraryPath: '/other/library', view: 'x', total: 1, rows: [{ cells: { id: 99, title: 'Other Library' } }] }),
    ),
  );
  await openApp(page, fixtureState());
  await expect(page.locator('[data-table-skeleton]')).toBeVisible();
  await expect(page.locator('[data-boot-row]')).toHaveCount(0);
  await releaseLoad(page);
  await page.locator('[data-track-row]').first().waitFor();
});

test('ratings changed outside the app since the last start land after the list shows', async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem(
      'fourfour.tableColumns.v3',
      JSON.stringify({
        layouts: {
          library: {
            columns: [
              { key: 'title', label: 'TITLE', width: 220 },
              { key: 'rating', label: 'RATING', width: 90 },
            ],
            sortKey: null,
            sortDir: 'asc',
          },
        },
      }),
    );
    (window as unknown as SmokeWindow).__smokeLabelChanges = [{ id: 2, rating: 4, color: null }];
  });
  await openApp(page, fixtureState());
  await releaseLoad(page);
  const row = page.locator('[data-track-row][data-row-index="1"]');
  await expect(row).toContainText('Smoke Track 02');
  await expect(row.locator('.ff-trow__star--on')).toHaveCount(4);
  await expect(page.locator('[data-track-row][data-row-index="0"] .ff-trow__star--on')).toHaveCount(0);
  const asked = await page.evaluate(() => (window as unknown as SmokeWindow).__smokeInvokes.filter((i) => i.cmd === 'refresh_track_labels').length);
  expect(asked).toBe(1);
});
