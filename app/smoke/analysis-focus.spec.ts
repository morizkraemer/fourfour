import { expect, test, type Page } from '@playwright/test';
import { largeFixtureState } from './fixture.ts';
import { installTauriMock, type SmokeWindow } from './tauri-mock.ts';

const TRACKS = 400;
const ANALYSED = new Set([201, 202, 203]);

function stateWithUnanalysedTracks() {
  const state = largeFixtureState(TRACKS);
  return {
    ...state,
    tracks: state.tracks.map((t) => ({ ...t, has_analysis: ANALYSED.has(t.id), tempo: ANALYSED.has(t.id) ? t.tempo : 0, key: ANALYSED.has(t.id) ? t.key : '' })),
  };
}

const focusCalls = (page: Page) =>
  page.evaluate(() =>
    (window as unknown as SmokeWindow).__smokeInvokes.filter((i) => i.cmd === 'focus_analysis').map((i) => (i.args as { ids: number[] }).ids),
  );

async function lastFocus(page: Page, check: (ids: number[]) => boolean) {
  let found: number[] = [];
  await expect
    .poll(async () => {
      const calls = await focusCalls(page);
      found = calls.at(-1) ?? [];
      return check(found);
    })
    .toBe(true);
  return found;
}

async function idsOnScreen(page: Page): Promise<number[]> {
  return page.evaluate(() => {
    const scroller = document.querySelector('.ff-table__scroll') as HTMLElement;
    const header = scroller.querySelector('.ff-colh') as HTMLElement | null;
    const top = scroller.getBoundingClientRect().top + (header?.offsetHeight ?? 0);
    const bottom = scroller.getBoundingClientRect().bottom;
    return [...scroller.querySelectorAll<HTMLElement>('[data-track-row]')]
      .filter((row) => {
        const box = row.getBoundingClientRect();
        return box.bottom > top + 1 && box.top < bottom - 1;
      })
      .sort((a, b) => a.getBoundingClientRect().top - b.getBoundingClientRect().top)
      .map((row) => Number(row.querySelector('.ff-trow__cell--title .ff-trow__text')?.textContent?.replace('Smoke Track ', '')));
  });
}

function expectOnScreenFirst(focus: number[], onScreen: number[]) {
  expect(onScreen).toContain(focus[0]);
  expect(focus.slice(0, onScreen.length + 1)).toEqual(expect.arrayContaining(onScreen));
}

test('the analysis is told which rows are on screen after a scroll and a sort', async ({ page }) => {
  await page.exposeFunction('__smokeAnalysis', () => null);
  await page.addInitScript(installTauriMock, stateWithUnanalysedTracks());
  await page.goto('/');
  await page.locator('[data-track-row]').first().waitFor();

  const first = await lastFocus(page, (ids) => ids.length > 0);
  expect(first[0]).toBe(1);

  await page.evaluate(() => {
    const scroller = document.querySelector('.ff-table__scroll') as HTMLElement;
    scroller.scrollTop = scroller.scrollHeight / 2;
  });
  const middle = await lastFocus(page, (ids) => ids[0] > 100);
  const onScreen = (await idsOnScreen(page)).filter((id) => !ANALYSED.has(id));
  expect(onScreen.length).toBeGreaterThan(5);
  expectOnScreenFirst(middle, onScreen);
  expect(middle.some((id) => ANALYSED.has(id))).toBe(false);
  expect(middle.length).toBeGreaterThan(onScreen.length);

  await page.locator('.ff-colh__cell[data-col-key="title"] .ff-colh__label').click();
  await page.locator('.ff-colh__cell[data-col-key="title"] .ff-colh__label').click();
  const sorted = await lastFocus(page, (ids) => ids.length > 0 && ids.join() !== middle.join());
  const sortedOnScreen = (await idsOnScreen(page)).filter((id) => !ANALYSED.has(id));
  expectOnScreenFirst(sorted, sortedOnScreen);

  const unhandled = await page.evaluate(() => (window as unknown as SmokeWindow).__smokeUnhandled);
  expect(unhandled).toEqual([]);
});
