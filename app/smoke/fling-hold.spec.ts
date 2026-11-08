import { test, expect, type Page } from '@playwright/test';
import { fixtureAnalysis, largeFixtureState } from './fixture.ts';
import { installTauriMock } from './tauri-mock.ts';

interface RowCells {
  index: number;
  title: string;
  artist: string;
  bpm: string;
  key: string;
}

function readVisibleRows(): RowCells[] {
  const scroller = document.querySelector('.ff-table__scroll') as HTMLElement;
  const view = scroller.getBoundingClientRect();
  const text = (row: Element, key: string) => row.querySelector(`.ff-trow__cell--${key} .ff-trow__text`)?.textContent ?? '';
  return [...document.querySelectorAll<HTMLElement>('[data-track-row]')]
    .filter((row) => {
      const box = row.getBoundingClientRect();
      return row.style.display !== 'none' && box.bottom > view.top && box.top < view.bottom;
    })
    .map((row) => ({ index: Number(row.dataset.rowIndex), title: text(row, 'title'), artist: text(row, 'artist'), bpm: text(row, 'bpm'), key: text(row, 'key') }));
}

const visibleRows = (page: Page) => page.evaluate(`(${readVisibleRows.toString()})()`) as Promise<RowCells[]>;

const flingFrames = (page: Page) =>
  page.evaluate(
    (read) =>
      new Promise<RowCells[][]>((resolve) => {
        const scroller = document.querySelector('.ff-table__scroll') as HTMLElement;
        const frames: RowCells[][] = [];
        const onScroll = () => requestAnimationFrame(() => frames.push((0, eval)(`(${read})()`)));
        scroller.addEventListener('scroll', onScroll);
        let frame = 0;
        const step = () => {
          scroller.scrollTop += 600;
          if (++frame < 12) requestAnimationFrame(step);
          else
            setTimeout(() => {
              scroller.removeEventListener('scroll', onScroll);
              resolve(frames);
            }, 400);
        };
        requestAnimationFrame(step);
      }),
    readVisibleRows.toString(),
  );

test('a fast fling shows title and artist at once and fills the other cells when it ends', async ({ page }) => {
  const state = largeFixtureState(3000);
  await page.exposeFunction('__smokeAnalysis', (trackId: number) => fixtureAnalysis(trackId));
  await page.addInitScript(installTauriMock, state);
  await page.goto('/');
  await page.locator('[data-track-row]').first().waitFor();

  const frames = await flingFrames(page);
  expect(frames.length).toBeGreaterThan(3);
  for (const rows of frames) {
    expect(rows.length).toBeGreaterThan(10);
    for (const row of rows) {
      const track = state.tracks[row.index];
      expect(row.title).toBe(track.title);
      expect(row.artist).toBe(track.artist);
    }
  }
  const held = frames.flat().filter((row) => row.bpm === '');
  expect(held.length, 'rows that changed track hold their other cells').toBeGreaterThan(5);
  expect(held.every((row) => row.key === '—')).toBe(true);

  await expect
    .poll(async () => (await visibleRows(page)).every((row) => row.bpm !== '' && row.key !== '—'))
    .toBe(true);
  for (const row of await visibleRows(page)) {
    const track = state.tracks[row.index];
    expect(row.title).toBe(track.title);
    expect(row.key).toBe(track.key);
  }
});

test('a slow scroll fills every cell as it goes', async ({ page }) => {
  await page.exposeFunction('__smokeAnalysis', (trackId: number) => fixtureAnalysis(trackId));
  await page.addInitScript(installTauriMock, largeFixtureState(3000));
  await page.goto('/');
  await page.locator('[data-track-row]').first().waitFor();
  const scroller = page.locator('.ff-table__scroll');
  await scroller.hover();
  for (let i = 0; i < 8; i++) {
    await page.mouse.wheel(0, 56);
    await page.waitForTimeout(60);
    expect((await visibleRows(page)).every((row) => row.bpm !== '' && row.key !== '—')).toBe(true);
  }
});
