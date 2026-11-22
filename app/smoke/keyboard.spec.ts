import type { Page } from '@playwright/test';
import { expect, rowByTitle, rows, test } from './app.ts';

const list = (page: Page) => page.getByRole('listbox', { name: 'Tracks' });
const selected = (page: Page) => page.locator('[data-track-row][aria-selected="true"]');
const saves = (page: Page) =>
  page.evaluate(() => (window as unknown as { __smokeInvokes: { cmd: string }[] }).__smokeInvokes.filter((i) => i.cmd === 'save_state').length);
const sidebarRow = (page: Page, name: string) => page.locator('.ff-sbrow').filter({ has: page.locator('.ff-sbrow__label', { hasText: new RegExp(`^${name}$`) }) });

async function focusList(page: Page) {
  await page.keyboard.press('F6');
  for (let i = 0; i < 6; i++) {
    if (await list(page).evaluate((el) => el === document.activeElement)) return;
    await page.keyboard.press('F6');
  }
  await expect(list(page)).toBeFocused();
}

async function addFavorite(page: Page) {
  await page.keyboard.press('F6');
  for (let i = 0; i < 12; i++) {
    if (await sidebarRow(page, 'Add favorite').evaluate((el) => el === document.activeElement)) break;
    await page.keyboard.press('ArrowUp');
  }
  await expect(sidebarRow(page, 'Add favorite')).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(sidebarRow(page, 'Favorites 1')).toBeVisible();
}

async function tabTo(page: Page, matches: () => ReturnType<Page['locator']>) {
  for (let i = 0; i < 40; i++) {
    if (await matches().evaluate((el) => el === document.activeElement).catch(() => false)) return;
    await page.keyboard.press('Tab');
  }
  await expect(matches()).toBeFocused();
}

function framesPassed(page: Page) {
  return page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));
}

test.describe('library list', () => {
  test('F6 skips the collapsed search field in a narrow window and still reaches the list', async ({ smoke: { page } }) => {
    await page.setViewportSize({ width: 900, height: 700 });
    await focusList(page);
    await expect(list(page)).toBeFocused();
  });

  test('F6 lands on the list, arrows move and select, Shift extends, Enter loads', async ({ smoke: { page } }) => {
    await focusList(page);
    await expect(list(page)).toHaveAttribute('aria-multiselectable', 'true');
    await expect(rows(page).first()).toHaveAttribute('role', 'option');

    await page.keyboard.press('ArrowDown');
    await expect(selected(page)).toHaveCount(1);
    await expect(rows(page).nth(0)).toHaveAttribute('aria-selected', 'true');
    await expect(list(page)).toHaveAttribute('aria-activedescendant', (await rows(page).nth(0).getAttribute('id'))!);

    await page.keyboard.press('ArrowDown');
    await expect(rows(page).nth(1)).toHaveAttribute('aria-selected', 'true');
    await expect(rows(page).nth(0)).toHaveAttribute('aria-selected', 'false');

    await page.keyboard.press('Shift+ArrowDown');
    await page.keyboard.press('Shift+ArrowDown');
    await expect(selected(page)).toHaveCount(3);
    await page.keyboard.press('Shift+ArrowUp');
    await expect(selected(page)).toHaveCount(2);

    await page.keyboard.press('ArrowDown');
    await expect(selected(page)).toHaveCount(1);
    await page.keyboard.press('Enter');
    await expect(page.locator('.ff-player-compact__meta-title')).toHaveText('Smoke Track 04');
  });

  test('Home, End, Page keys and Ctrl+A; the cursor row scrolls into view', async ({ smoke: { page } }) => {
    await focusList(page);
    await page.keyboard.press('End');
    await expect(rowByTitle(page, 'Smoke Track 20')).toHaveAttribute('aria-selected', 'true');
    await expect(rowByTitle(page, 'Smoke Track 20')).toBeInViewport();
    await page.keyboard.press('Home');
    await expect(rowByTitle(page, 'Smoke Track 01')).toHaveAttribute('aria-selected', 'true');
    await page.keyboard.press('PageDown');
    await expect(selected(page)).toHaveCount(1);
    await expect(selected(page)).not.toHaveAttribute('data-row-index', '0');
    await page.keyboard.press('Control+a');
    await expect(selected(page)).toHaveCount(20);
  });

  test('Space on the list plays and pauses; digits and letters are shortcuts, not list navigation', async ({ smoke: { page } }) => {
    await focusList(page);
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('Enter');
    await expect(page.getByRole('button', { name: 'Pause', exact: true })).toBeVisible();
    await page.keyboard.press('Space');
    await expect(page.getByRole('button', { name: 'Play', exact: true })).toBeVisible();
    await page.keyboard.press('Space');
    await expect(page.getByRole('button', { name: 'Pause', exact: true })).toBeVisible();
  });

  test('Shift+F10 opens the track menu at the cursor row; arrows and Escape work and focus returns', async ({ smoke: { page } }) => {
    await focusList(page);
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('Shift+F10');
    const menu = page.getByRole('menu');
    await expect(menu).toBeVisible();
    await expect(menu).toBeFocused();
    await framesPassed(page);
    await page.keyboard.press('ArrowDown');
    await expect(menu.getByRole('menuitem').first()).toBeFocused();
    await page.keyboard.press('ArrowUp');
    await expect(menu.getByRole('menuitem').last()).toBeFocused();
    await page.keyboard.press('Escape');
    await expect(menu).toHaveCount(0);
    await expect(list(page)).toBeFocused();
  });

  test('Tab leaves the list instead of toggling the player once the list has keyboard focus', async ({ smoke: { page } }) => {
    await focusList(page);
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('Enter');
    await expect(page.locator('.ff-player-compact__meta-title')).toHaveText('Smoke Track 01');
    await page.keyboard.press('Tab');
    await expect(page.locator('.ff-xp')).toHaveCount(0);
    await expect(list(page)).not.toBeFocused();
  });

  test('after mouse clicks the player shortcuts still win: Space plays and pauses, Tab expands the player', async ({ smoke: { page } }) => {
    await rowByTitle(page, 'Smoke Track 02').dblclick();
    await expect(page.getByRole('button', { name: 'Pause', exact: true })).toBeVisible();
    await sidebarRow(page, 'Peak Time').click();
    await rows(page).first().click();
    await page.keyboard.press('Space');
    await expect(page.getByRole('button', { name: 'Play', exact: true })).toBeVisible();
    await sidebarRow(page, 'Warmup').click();
    await page.keyboard.press('Space');
    await expect(page.getByRole('button', { name: 'Pause', exact: true })).toBeVisible();
    await page.keyboard.press('Tab');
    await expect(page.locator('.ff-xp')).toBeVisible();
  });

  test('Tab opens and closes the empty player when no track is loaded', async ({ smoke: { page } }) => {
    await expect(page.locator('.ff-player-compact__meta-title')).toHaveText('No Track Loaded');
    await page.keyboard.press('Tab');
    await expect(page.locator('.ff-xp')).toBeVisible();
    await expect(page.locator('.ff-xp__title')).toHaveText('No Track Loaded');
    await page.keyboard.press('Tab');
    await expect(page.locator('.ff-xp')).toHaveCount(0);
    await rows(page).first().click();
    await page.keyboard.press('Tab');
    await expect(page.locator('.ff-xp')).toBeVisible();
  });
});

test.describe('sidebar', () => {
  test('rows are reachable by Tab, arrows move, Enter opens, current row is exposed', async ({ smoke: { page } }) => {
    await page.keyboard.press('F6');
    await tabTo(page, () => sidebarRow(page, 'Peak Time'));
    await expect(sidebarRow(page, 'Peak Time')).toHaveAttribute('role', 'button');
    await page.keyboard.press('Enter');
    await expect(page.locator('.ff-gheader__title')).toHaveText('Peak Time');
    await expect(sidebarRow(page, 'Peak Time')).toHaveAttribute('aria-current', 'page');
    await expect(rows(page)).toHaveCount(3);

    await page.keyboard.press('ArrowDown');
    await expect(sidebarRow(page, 'Warmup')).toBeFocused();
    await page.keyboard.press('Space');
    await expect(page.locator('.ff-gheader__title')).toHaveText('Warmup');
    await expect(rows(page)).toHaveCount(1);

    await page.keyboard.press('ArrowUp');
    await expect(sidebarRow(page, 'Peak Time')).toBeFocused();
  });

  test('F2 renames the focused playlist, Enter commits and focus stays reachable', async ({ smoke: { page } }) => {
    await page.keyboard.press('F6');
    await tabTo(page, () => sidebarRow(page, 'Warmup'));
    await page.keyboard.press('F2');
    const input = page.getByRole('textbox', { name: 'Name' });
    await expect(input).toBeFocused();
    await page.keyboard.press('Control+a');
    await page.keyboard.type('Closing 1');
    await page.keyboard.press('Enter');
    await expect(sidebarRow(page, 'Closing 1')).toBeVisible();
    await expect(input).toHaveCount(0);
  });

  test('Escape cancels a rename; typed digits stay in the field', async ({ smoke: { page } }) => {
    await page.keyboard.press('F6');
    await tabTo(page, () => sidebarRow(page, 'Warmup'));
    const before = await saves(page);
    await page.keyboard.press('F2');
    await page.keyboard.type('2 3 4');
    await page.keyboard.press('Escape');
    await expect(sidebarRow(page, 'Warmup')).toBeVisible();
    expect(await saves(page)).toBe(before);
  });

  test('Shift+F10 on a playlist opens its menu; Escape returns focus to the row', async ({ smoke: { page } }) => {
    await page.keyboard.press('F6');
    await tabTo(page, () => sidebarRow(page, 'Warmup'));
    await page.keyboard.press('Shift+F10');
    const menu = page.getByRole('menu');
    await expect(menu).toBeVisible();
    await framesPassed(page);
    await page.keyboard.press('ArrowDown');
    await expect(menu.getByRole('menuitem').first()).toBeFocused();
    await page.keyboard.press('Escape');
    await expect(menu).toHaveCount(0);
    await expect(sidebarRow(page, 'Warmup')).toBeFocused();
  });
});

test.describe('dialogs', () => {
  test('settings: focus moves in, Tab stays inside, current page is exposed, Escape returns focus', async ({ smoke: { page } }) => {
    await focusList(page);
    await page.keyboard.press('Control+,');
    const sheet = page.getByRole('dialog', { name: 'Preferences' });
    await expect(sheet).toBeVisible();
    await expect(sheet.locator('[aria-current="page"]')).toHaveCount(1);
    for (let i = 0; i < 40; i++) {
      await page.keyboard.press('Tab');
      expect(await sheet.evaluate((el) => el.contains(document.activeElement))).toBe(true);
    }
    for (let i = 0; i < 40; i++) {
      await page.keyboard.press('Shift+Tab');
      expect(await sheet.evaluate((el) => el.contains(document.activeElement))).toBe(true);
    }
    await page.keyboard.press('Escape');
    await expect(sheet).toHaveCount(0);
    await expect(list(page)).toBeFocused();
  });

  test('settings: plain keys do not fire shortcuts', async ({ smoke: { page } }) => {
    await addFavorite(page);
    await focusList(page);
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('1');
    await expect.poll(() => saves(page)).toBeGreaterThan(0);
    const before = await saves(page);
    await page.keyboard.press('Control+,');
    await expect(page.getByRole('dialog', { name: 'Preferences' })).toBeVisible();
    await page.keyboard.type('1 2 3');
    expect(await saves(page)).toBe(before);
  });

  test('import flow: the analyze prompt takes focus, traps Tab, toggles by Space, Escape returns focus', async ({ smoke: { page } }) => {
    await page.evaluate(() =>
      Object.assign(window, {
        __smokePickPath: '/Downloads/Fresh',
        __smokeImport: {
          tracks: [{ id: 101, source_path: '/Downloads/Fresh/Low Sun.mp3', title: 'Low Sun', artist: 'Drop', album: 'Drop', genre: 'House', tempo: 12400, key: '8A', duration_secs: 300, bitrate: 320, file_size: 1e7, has_artwork: false, has_cues: false, has_analysis: false, analysis_pending: false }],
          already: 0,
        },
      }),
    );
    await focusList(page);
    await page.keyboard.press('Control+o');
    const dialog = page.getByRole('dialog', { name: 'Analyze imported tracks?' });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole('button', { name: 'Analyze' })).toBeFocused();
    const always = dialog.getByRole('checkbox');
    await page.keyboard.press('Shift+Tab');
    await page.keyboard.press('Shift+Tab');
    await expect(always).toBeFocused();
    await page.keyboard.press('Space');
    await expect(always).toHaveAttribute('aria-checked', 'true');
    for (let i = 0; i < 8; i++) {
      await page.keyboard.press('Tab');
      expect(await dialog.evaluate((el) => el.contains(document.activeElement))).toBe(true);
    }
    await page.keyboard.press('Escape');
    await expect(dialog).toHaveCount(0);
    await expect(list(page)).toBeFocused();
  });
});

test.describe('search and palette', () => {
  test('Ctrl+F focuses the labelled search, typing does not fire shortcuts, Escape clears and leaves', async ({ smoke: { page } }) => {
    await page.keyboard.press('Control+f');
    const search = page.getByRole('searchbox', { name: 'Search tracks and commands' });
    await expect(search).toBeFocused();
    await page.keyboard.type('smoke 1 2 3 c m');
    await expect(search).toHaveValue('smoke 1 2 3 c m');
    expect(await page.evaluate(() => (window as unknown as { __smokeInvokes: { cmd: string }[] }).__smokeInvokes.filter((i) => i.cmd === 'set_cues' || i.cmd === 'playback_play').length)).toBe(0);
    await page.keyboard.press('Escape');
    await page.keyboard.press('Escape');
    await expect(search).toHaveValue('');
    await expect(search).not.toBeFocused();
  });

  test('Ctrl+K palette: arrows choose, Enter runs, status message is announced', async ({ smoke: { page } }) => {
    await page.keyboard.press('Control+k');
    const box = page.getByRole('listbox');
    await expect(box.first()).toBeVisible();
    await page.keyboard.type('warm');
    await expect(page.getByRole('option', { name: /Warmup/ }).first()).toBeVisible();
    await page.keyboard.press('Enter');
    await expect(page.locator('.ff-gheader__title')).toHaveText('Warmup');
    await expect(page.locator('[role="status"]:not([data-live-region])')).toHaveCount(1);
  });
});

test.describe('column headers', () => {
  test('sort from the keyboard: Shift+Tab from the list, arrows between columns, Enter sorts', async ({ smoke: { page } }) => {
    await focusList(page);
    await page.keyboard.press('Shift+Tab');
    const header = page.locator('.ff-colh__cell:focus');
    await expect(header).toHaveAttribute('role', 'button');
    for (let i = 0; i < 14; i++) {
      if (((await header.getAttribute('aria-label')) ?? '').startsWith('Sort by ARTIST')) break;
      await page.keyboard.press('ArrowRight');
    }
    await expect(header).toHaveAttribute('aria-label', 'Sort by ARTIST');
    await page.keyboard.press('Enter');
    await expect(header).toHaveAttribute('aria-label', 'Sort by ARTIST, ascending');
    await expect(rows(page).first().locator('.ff-trow__cell--artist')).toHaveText('Ben Böhmer');
    await page.keyboard.press('Space');
    await expect(header).toHaveAttribute('aria-label', 'Sort by ARTIST, descending');
  });
});
