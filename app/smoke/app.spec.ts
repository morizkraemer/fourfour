import { TRACK_COUNT } from './fixture.ts';
import { dragTo, expect, loadByDoubleClick, rowByTitle, rows, rowTitles, sidebarPlaylist, test } from './app.ts';

const TRACK = 'Smoke Track 03';
const OTHER_TRACK = 'Smoke Track 05';

test('boots and renders the library table', async ({ smoke: { page } }) => {
  await expect(rows(page)).not.toHaveCount(0);
  await expect(page.locator('.ff-gheader__count')).toHaveText(String(TRACK_COUNT));
  await expect(rowByTitle(page, 'Smoke Track 01')).toBeVisible();
  await expect(page.locator('.ff-sbrow__label', { hasText: 'Peak Time' })).toBeVisible();
  await expect(page.locator('.ff-player-compact__meta-title')).toHaveText('No Track Loaded');
});

test('sorts by a column header and reverses on a second click', async ({ smoke: { page } }) => {
  const initial = await rowTitles(page);
  await page.locator('.ff-colh__cell[data-col-key="title"] .ff-colh__label').click();
  const first = await rowTitles(page);
  await page.locator('.ff-colh__cell[data-col-key="title"] .ff-colh__label').click();
  const second = await rowTitles(page);
  expect(first).toEqual([...first].sort((a, b) => a.localeCompare(b, undefined, { numeric: true })));
  expect(second).toEqual([...first].reverse());
  expect(second).not.toEqual(initial);
});

test('search filters the table rows', async ({ smoke: { page } }) => {
  await page.locator('.ff-gheader__search input').fill('Track 07');
  await expect(rows(page)).not.toHaveCount(TRACK_COUNT);
  await expect(rowByTitle(page, 'Smoke Track 07')).toBeVisible();
  await page.locator('.ff-gheader__search input').fill('');
  await expect(rows(page)).toHaveCount(TRACK_COUNT);
});

test('selecting a track shows it in the inspector', async ({ smoke: { page } }) => {
  await rowByTitle(page, TRACK).click();
  const inspector = page.locator('.ff-inspector');
  await expect(inspector.locator('input[aria-label="Title"]')).toHaveValue(TRACK);
  await expect(inspector.locator('[data-fig="bpm"] .ff-scrub__text')).toHaveText('126.00');
  await expect(inspector.locator('[data-fig="key"] .ff-scrub__text')).toHaveText('5A');
  await expect(inspector.locator('[data-marker-key="h1"]')).toBeVisible();
});

test('double click loads a track into the player', async ({ smoke: { page } }) => {
  await loadByDoubleClick(page, TRACK);
  await expect(page.locator('.ff-player-compact__meta-artist')).toHaveText('Bicep');
  await expect(page.locator('.ff-player-compact__time-tot')).toHaveText('3:14');
});

test('dragging a track onto the player loads it', async ({ smoke: { page } }) => {
  await dragTo(page, rowByTitle(page, OTHER_TRACK), page.locator('[data-drop-player]'), page.locator('.ff-player-module--drop'));
  await expect(page.locator('.ff-player-compact__meta-title')).toHaveText(OTHER_TRACK);
});

test('play and pause toggle the transport', async ({ smoke: { page } }) => {
  await loadByDoubleClick(page, TRACK);
  const play = page.locator('.ff-player-compact__play-btn');
  await expect(play).toHaveAttribute('aria-label', 'Pause');
  await expect(page.locator('.ff-player-compact__time-cur')).not.toHaveText(/^0:00\.\d$/);
  await play.click();
  await expect(play).toHaveAttribute('aria-label', 'Play');
  await page.keyboard.press('Space');
  await expect(play).toHaveAttribute('aria-label', 'Pause');
});

test('expands the big player and collapses it with Escape', async ({ smoke: { page } }) => {
  await loadByDoubleClick(page, TRACK);
  await page.locator('.ff-player-compact__expand-btn').click();
  await expect(page.locator('.ff-xp')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.locator('.ff-xp')).toHaveCount(0);
  await expect(page.locator('.ff-player-compact')).toBeVisible();
});

test('the BPM pencil opens its popup without shifting the header, and saving re-spaces the grid', async ({ smoke }) => {
  const { page } = smoke;
  await loadByDoubleClick(page, TRACK);
  await page.locator('.ff-player-compact__expand-btn').click();
  await expect(page.locator('.ff-xp')).toBeVisible();

  const group = page.locator('.ff-xp .ff-ptempo--xp');
  const fieldBox = async (name: string) => (await group.locator(`[role="group"][aria-label="${name}"]`).boundingBox())!;
  const pads = page.locator('.ff-xp__pads');
  const padsBefore = await pads.boundingBox();
  const bpmBefore = await fieldBox('BPM');
  const keyBefore = await fieldBox('Key');

  await group.getByRole('button', { name: 'Edit track BPM' }).click();
  const popup = page.getByRole('dialog', { name: 'Track BPM' });
  await expect(popup.getByRole('textbox', { name: 'Track BPM' })).toHaveValue('126.00');
  expect(await fieldBox('BPM')).toEqual(bpmBefore);
  expect(await fieldBox('Key')).toEqual(keyBefore);
  expect(await pads.boundingBox()).toEqual(padsBefore);

  await popup.getByRole('button', { name: 'Halve track BPM' }).click();
  await popup.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(popup).toHaveCount(0);
  await expect(rowByTitle(page, TRACK).locator('[data-col-key="bpm"]')).toContainText('63.0');
  await expect.poll(async () => (await smoke.invokes('rescale_beat_grid')).length + (await smoke.invokes('set_beat_grid')).length).toBeGreaterThan(0);
  await expect(page.locator('.ff-xp')).toBeVisible();
  expect(await fieldBox('BPM')).toEqual(bpmBefore);
});

test('memory cue chips add at the playhead, jump and remove with Alt', async ({ smoke }) => {
  const { page } = smoke;
  await loadByDoubleClick(page, TRACK);
  await page.locator('.ff-player-compact__expand-btn').click();
  await expect(page.locator('.ff-xp__pads .ff-xp__pad--set')).toHaveCount(1);
  const chips = page.locator('.ff-xp [data-memory-chip]');
  const before = await chips.count();
  await page.locator('.ff-xp').getByRole('button', { name: 'Add memory cue', exact: true }).click();
  await expect(chips).toHaveCount(before + 1);
  await expect.poll(async () => (await smoke.invokes('set_cues')).length).toBeGreaterThan(0);
  await chips.last().click({ modifiers: ['Alt'] });
  await expect(chips).toHaveCount(before);
});

test('adds the next free hot cue with Shift+C and removes one with Alt', async ({ smoke }) => {
  const { page } = smoke;
  await loadByDoubleClick(page, TRACK);
  await page.locator('.ff-player-compact__expand-btn').click();
  const pads = page.locator('.ff-xp__pads');
  await expect(pads.locator('.ff-xp__pad--set')).toHaveCount(1);
  await expect(pads.locator('[data-pad="A"]')).toHaveClass(/ff-xp__pad--set/);

  await page.keyboard.press('Shift+C');
  await expect(pads.locator('.ff-xp__pad--set')).toHaveCount(2);
  await expect(pads.locator('[data-pad="B"]')).toHaveClass(/ff-xp__pad--set/);
  await expect.poll(async () => (await smoke.invokes('set_cues')).length).toBeGreaterThan(0);

  await pads.locator('[data-pad="B"]').click({ modifiers: ['Alt'] });
  await expect(pads.locator('[data-pad="B"]')).not.toHaveClass(/ff-xp__pad--set/);
  await expect(pads.locator('.ff-xp__pad--set')).toHaveCount(1);
});

test('command palette opens with Ctrl+K, filters and runs a command', async ({ smoke: { page } }) => {
  await page.keyboard.press('Control+k');
  const palette = page.locator('.ff-cp');
  await expect(palette).toBeVisible();
  const allRows = await palette.locator('.ff-cp__row').count();
  expect(allRows).toBeGreaterThan(3);

  await page.keyboard.type('keyboard shortcuts');
  await expect(palette.locator('.ff-cp__row')).toHaveCount(1);
  await expect(palette.locator('.ff-cp__row-name')).toContainText('Show keyboard shortcuts');
  await page.keyboard.press('Enter');
  await expect(palette).toHaveCount(0);
  await expect(page.locator('.ff-settings, [role="dialog"]').first()).toBeVisible();
});

test('creates, renames and fills a playlist by drag, then deletes it after confirming', async ({ smoke }) => {
  const { page } = smoke;
  await page.keyboard.press('Control+Shift+N');
  const rename = page.locator('.ff-sbrow__label-input');
  await expect(rename).toBeVisible();
  await rename.fill('Smoke List');
  await rename.press('Enter');
  const playlist = sidebarPlaylist(page, 'Smoke List');
  await expect(playlist).toBeVisible();
  await expect(playlist.locator('.ff-sbrow__count')).toHaveText('0');

  await playlist.locator('.ff-sbrow__label').dblclick();
  await page.locator('.ff-sbrow__label-input').fill('Smoke Renamed');
  await page.keyboard.press('Enter');
  const renamed = sidebarPlaylist(page, 'Smoke Renamed');
  await expect(renamed).toBeVisible();

  await page.locator('.ff-sbrow__label', { hasText: 'All Tracks' }).click();
  await dragTo(page, rowByTitle(page, TRACK), renamed, page.locator('.ff-sidebar__drop-wrap.drag-over'));
  await expect(renamed.locator('.ff-sbrow__count')).toHaveText('1');
  await expect.poll(async () => (await smoke.invokes('save_state')).length).toBeGreaterThan(0);

  await smoke.answerDialogs(false);
  await renamed.click({ button: 'right' });
  await page.locator('.ff-menu__item', { hasText: 'Delete playlist' }).click();
  await expect.poll(() => smoke.dialogs()).toHaveLength(1);
  await expect(renamed).toBeVisible();

  await smoke.answerDialogs(true);
  await renamed.click({ button: 'right' });
  await page.locator('.ff-menu__item', { hasText: 'Delete playlist' }).click();
  await expect(renamed).toHaveCount(0);
  expect((await smoke.dialogs())[1]).toContain('Smoke Renamed');
});
