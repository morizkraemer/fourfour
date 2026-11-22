import { expect, test, sidebarPlaylist } from './app.ts';

test('Export playlist picks a format, asks where to save and reports the result', async ({ smoke }) => {
  const { page } = smoke;
  await sidebarPlaylist(page, 'Peak Time').click({ button: 'right' });
  await page.locator('.ff-menu__item', { hasText: 'Export playlist…' }).hover();
  await expect(page.locator('.ff-menu__item')).toContainText([
    'M3U8 playlist',
    'CSV spreadsheet',
    'Text tracklist',
    'Traktor (NML)',
    'Rekordbox XML',
  ]);
  await page.getByRole('menuitem', { name: 'Rekordbox XML' }).click();

  await expect(page.locator('.ff-statusbar-module')).toContainText(
    'Exported 3 tracks to Peak Time 2026-11-21.xml · 1 audio file not found, still listed'
  );
  expect(await smoke.invokes('export_playlist')).toEqual([
    { playlistId: 1, format: 'rekordbox_xml', destPath: '/Backups/Peak Time 2026-11-21.xml' },
  ]);
});

test('Copy tracklist reports how many tracks it copied', async ({ smoke }) => {
  const { page } = smoke;
  await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
  await sidebarPlaylist(page, 'Peak Time').click({ button: 'right' });
  await page.locator('.ff-menu__item', { hasText: 'Copy tracklist' }).click();

  await expect(page.locator('.ff-statusbar-module')).toContainText('Copied a tracklist of 2 tracks');
  expect(await smoke.invokes('playlist_tracklist')).toEqual([{ playlistId: 1 }]);
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe('Artist One - Track One\nArtist Two - Track Two\n');
});
