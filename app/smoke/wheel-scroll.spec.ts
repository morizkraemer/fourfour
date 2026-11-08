import { test, expect } from '@playwright/test';
import { fixtureAnalysis, largeFixtureState } from './fixture.ts';
import { installTauriMock } from './tauri-mock.ts';

interface FrameCheck {
  top: number;
  missing: number;
  incomplete: number;
}

function probeFrames() {
  const w = window as unknown as { __frames: FrameCheck[]; __probe: boolean };
  w.__frames = [];
  w.__probe = false;
  const frame = () => {
    requestAnimationFrame(frame);
    if (!w.__probe) return;
    const scroller = document.querySelector('.ff-table__scroll') as HTMLElement | null;
    if (!scroller) return;
    const header = (scroller.querySelector('.ff-colh') as HTMLElement).offsetHeight;
    const top = scroller.scrollTop;
    const first = Math.floor(top / 28);
    const last = Math.ceil((top + scroller.clientHeight - header) / 28) - 1;
    const rows = new Map<number, HTMLElement>();
    for (const row of document.querySelectorAll<HTMLElement>('[data-track-row]')) {
      if (row.style.display !== 'none') rows.set(Number(row.dataset.rowIndex), row);
    }
    let missing = 0;
    let incomplete = 0;
    for (let i = first; i <= last; i++) {
      const row = rows.get(i);
      if (!row?.querySelector('.ff-trow__cell--title .ff-trow__text')?.textContent) missing++;
      else if (!row.querySelector('.ff-trow__cell--bpm .ff-trow__text')?.textContent) incomplete++;
    }
    w.__frames.push({ top, missing, incomplete });
  };
  requestAnimationFrame(frame);
}

test('a trackpad-speed wheel fling moves the list together with its rows: no blank or half-filled row in any frame', async ({ page }) => {
  await page.exposeFunction('__smokeAnalysis', (trackId: number) => fixtureAnalysis(trackId));
  await page.addInitScript(installTauriMock, largeFixtureState(3000));
  await page.addInitScript(probeFrames);
  await page.goto('/');
  await page.locator('[data-track-row]').first().waitFor();
  await page.locator('.ff-table__scroll').hover();
  await page.evaluate(() => ((window as unknown as { __probe: boolean }).__probe = true));

  const steps = 40;
  const delta = 120;
  for (let i = 0; i < steps; i++) {
    await page.mouse.wheel(0, delta);
    await page.waitForTimeout(16);
  }
  await expect.poll(() => page.evaluate(() => (document.querySelector('.ff-table__scroll') as HTMLElement).scrollTop)).toBe(steps * delta);
  await page.waitForTimeout(200);

  const frames = await page.evaluate(() => (window as unknown as { __frames: FrameCheck[] }).__frames);
  const moving = frames.filter((f, i) => i > 0 && f.top !== frames[i - 1].top);
  expect(moving.length, 'frames in which the list moved').toBeGreaterThan(3);
  expect(frames.filter((f) => f.missing > 0), 'frames with an empty row slot on screen').toEqual([]);
  expect(frames.filter((f) => f.incomplete > 0), 'frames with a row missing its other cells').toEqual([]);
});

test('Ctrl+wheel (pinch zoom) is left to the browser', async ({ page }) => {
  await page.exposeFunction('__smokeAnalysis', (trackId: number) => fixtureAnalysis(trackId));
  await page.addInitScript(installTauriMock, largeFixtureState(300));
  await page.goto('/');
  await page.locator('[data-track-row]').first().waitFor();
  const scroller = page.locator('.ff-table__scroll');
  await scroller.hover();
  const prevented = await page.evaluate(
    () =>
      new Promise<boolean>((resolve) => {
        const el = document.querySelector('.ff-table__scroll') as HTMLElement;
        window.addEventListener('wheel', (e) => resolve(e.defaultPrevented), { once: true });
        el.dispatchEvent(new WheelEvent('wheel', { deltaY: 100, ctrlKey: true, bubbles: true, cancelable: true }));
      }),
  );
  expect(prevented).toBe(false);
});
