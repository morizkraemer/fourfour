import type { Page } from '@playwright/test';
import { expect, rowByTitle, rowTitles, rows, test } from './app.ts';

const list = (page: Page) => page.getByRole('listbox', { name: 'Tracks' });
const liveRegion = (page: Page) => page.locator('[data-live-region]');
const sidebarRow = (page: Page, name: string) => page.locator('.ff-sbrow').filter({ has: page.locator('.ff-sbrow__label', { hasText: new RegExp(`^${name}$`) }) });
const slider = (page: Page) => page.locator('[data-wave-slider]');

type Invokes = { invokes(cmd: string): Promise<Record<string, any>[]> };

async function focusList(page: Page) {
  await page.keyboard.press('F6');
  for (let i = 0; i < 6; i++) {
    if (await list(page).evaluate((el) => el === document.activeElement)) return;
    await page.keyboard.press('F6');
  }
  await expect(list(page)).toBeFocused();
}

test.describe('rating by keys', () => {
  test('Mod+1 to Mod+5 rate the selected track, Mod+0 clears, the change is announced', async ({ smoke }) => {
    const { page } = smoke;
    await focusList(page);
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('Control+3');
    await expect.poll(async () => (await smoke.invokes('set_track_rating')).length).toBe(1);
    expect((await smoke.invokes('set_track_rating'))[0]).toMatchObject({ trackId: 1, rating: 3 });
    await expect(liveRegion(page)).toHaveText('Rated 3 stars');

    await page.keyboard.press('Control+5');
    await expect.poll(async () => (await smoke.invokes('set_track_rating')).length).toBe(2);
    expect((await smoke.invokes('set_track_rating'))[1]).toMatchObject({ trackId: 1, rating: 5 });

    await page.keyboard.press('Control+0');
    await expect.poll(async () => (await smoke.invokes('set_track_rating')).length).toBe(3);
    expect((await smoke.invokes('set_track_rating'))[2]).toMatchObject({ trackId: 1, rating: 0 });
    await expect(liveRegion(page)).toHaveText('Rating cleared');
  });

  test('rates every selected track', async ({ smoke }) => {
    const { page } = smoke;
    await focusList(page);
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('Shift+ArrowDown');
    await page.keyboard.press('Shift+ArrowDown');
    await page.keyboard.press('Control+4');
    await expect.poll(async () => (await smoke.invokes('set_track_rating')).length).toBe(3);
    const saved = await smoke.invokes('set_track_rating');
    expect(saved.map((s) => s.trackId).sort()).toEqual([1, 2, 3]);
    expect(saved.every((s) => s.rating === 4)).toBe(true);
    await expect(liveRegion(page)).toHaveText('Rated 4 stars on 3 tracks');
  });

  test('plain digits keep toggling favorites and never rate; outside the list the keys do nothing', async ({ smoke }) => {
    const { page } = smoke;
    await focusList(page);
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('3');
    await sidebarRow(page, 'Warmup').focus();
    await expect(list(page)).not.toBeFocused();
    await page.keyboard.press('Control+3');
    await page.waitForTimeout(150);
    expect(await smoke.invokes('set_track_rating')).toEqual([]);
  });
});

test.describe('waveform seek by keys', () => {
  async function loadPaused(page: Page) {
    await focusList(page);
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('Enter');
    await expect(page.locator('.ff-player-compact__meta-title')).toHaveText('Smoke Track 03');
    await page.keyboard.press('Space');
    await expect(page.getByRole('button', { name: 'Play', exact: true })).toBeVisible();
  }

  test('the compact waveform is a slider; arrows seek by beat, Shift by four bars, Home and End jump', async ({ smoke: { page } }) => {
    await loadPaused(page);
    await expect(slider(page)).toHaveAttribute('role', 'slider');
    await expect(slider(page)).toHaveAttribute('aria-valuemin', '0');
    await expect(slider(page)).toHaveAttribute('aria-valuemax', '194');
    await slider(page).focus();
    await page.keyboard.press('Home');
    await expect(slider(page)).toHaveAttribute('aria-valuetext', '0:00 of 3:14');

    await page.keyboard.press('Shift+ArrowRight');
    await expect(slider(page)).toHaveAttribute('aria-valuetext', '0:07 of 3:14');
    await expect(slider(page)).toHaveAttribute('aria-valuenow', '7');
    await page.keyboard.press('Shift+ArrowRight');
    await expect(slider(page)).toHaveAttribute('aria-valuetext', '0:15 of 3:14');
    await page.keyboard.press('Shift+ArrowLeft');
    await expect(slider(page)).toHaveAttribute('aria-valuetext', '0:07 of 3:14');
    for (let i = 0; i < 4; i++) await page.keyboard.press('ArrowRight');
    await expect(slider(page)).toHaveAttribute('aria-valuetext', '0:09 of 3:14');
    await page.keyboard.press('ArrowLeft');
    await expect(slider(page)).toHaveAttribute('aria-valuetext', '0:09 of 3:14');
    await page.keyboard.press('End');
    await expect(slider(page)).toHaveAttribute('aria-valuetext', '3:14 of 3:14');
    await page.keyboard.press('Home');
    await expect(slider(page)).toHaveAttribute('aria-valuetext', '0:00 of 3:14');
  });

  test('the expanded waveform is a slider too, and arrows outside a focused slider do not seek', async ({ smoke: { page } }) => {
    await loadPaused(page);
    await page.locator('.ff-player-compact__expand-btn').click();
    await expect(page.locator('.ff-xp')).toBeVisible();
    const wave = page.locator('.ff-xp__wave');
    await expect(wave).toHaveAttribute('role', 'slider');
    await wave.focus();
    await page.keyboard.press('Shift+ArrowRight');
    await expect(wave).toHaveAttribute('aria-valuetext', '0:07 of 3:14');
    await page.keyboard.press('Home');
    await expect(wave).toHaveAttribute('aria-valuetext', '0:00 of 3:14');

    await page.keyboard.press('Escape');
    await expect(page.locator('.ff-xp')).toHaveCount(0);
    await focusList(page);
    await page.keyboard.press('ArrowRight');
    await page.keyboard.press('ArrowDown');
    await expect(slider(page)).toHaveAttribute('aria-valuetext', '0:00 of 3:14');
  });
});

test.describe('keyboard reorder in a playlist', () => {
  const savedOrder = async (smoke: Invokes, name: string) => {
    const saves = await smoke.invokes('save_state');
    return (saves.at(-1)?.playlists as { name: string; track_ids: number[] }[]).find((p) => p.name === name)!.track_ids;
  };

  test('Alt+Down and Alt+Up move the selected track one row, save, keep focus and announce', async ({ smoke }) => {
    const { page } = smoke;
    await sidebarRow(page, 'Peak Time').click();
    await expect(rows(page)).toHaveCount(3);
    const before = await rowTitles(page);
    await focusList(page);
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('Alt+ArrowDown');
    await expect.poll(() => rowTitles(page)).toEqual([before[1], before[0], before[2]]);
    await expect(list(page)).toBeFocused();
    await expect(liveRegion(page)).toHaveText(`Moved ${before[0]} down, position 2 of 3`);
    const afterDown = await savedOrder(smoke, 'Peak Time');
    expect(afterDown).toHaveLength(3);

    await page.keyboard.press('Alt+ArrowDown');
    await expect.poll(() => rowTitles(page)).toEqual([before[1], before[2], before[0]]);
    await page.keyboard.press('Alt+ArrowDown');
    await expect(liveRegion(page)).toHaveText('Already at the bottom');
    await expect.poll(() => rowTitles(page)).toEqual([before[1], before[2], before[0]]);

    await page.keyboard.press('Alt+ArrowUp');
    await page.keyboard.press('Alt+ArrowUp');
    await expect.poll(() => rowTitles(page)).toEqual(before);
    await page.keyboard.press('Alt+ArrowUp');
    await expect(liveRegion(page)).toHaveText('Already at the top');
    await expect.poll(() => savedOrder(smoke, 'Peak Time')).toEqual([afterDown[1], afterDown[0], afterDown[2]]);
  });

  test('a multi-track selection moves together', async ({ smoke }) => {
    const { page } = smoke;
    await sidebarRow(page, 'Peak Time').click();
    const before = await rowTitles(page);
    await focusList(page);
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('Shift+ArrowDown');
    await page.keyboard.press('Alt+ArrowDown');
    await expect.poll(() => rowTitles(page)).toEqual([before[2], before[0], before[1]]);
    await expect(liveRegion(page)).toHaveText('Moved 2 tracks down, cursor at position 3 of 3');
  });

  test('the library view is not reorderable: Alt+Down changes nothing', async ({ smoke }) => {
    const { page } = smoke;
    const before = await rowTitles(page);
    const savesBefore = (await smoke.invokes('save_state')).length;
    await focusList(page);
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('Alt+ArrowDown');
    await page.waitForTimeout(150);
    expect(await rowTitles(page)).toEqual(before);
    expect((await smoke.invokes('save_state')).length).toBe(savesBefore);
    await expect(rowByTitle(page, before[0])).toHaveAttribute('aria-selected', 'true');
  });
});
