import type { Page } from '@playwright/test';
import type { Scenario } from './types.ts';

const chip = '[data-library-status]';
const menu = '[data-library-activity-menu]';
const CHIP = { x: 1040, y: 0, width: 400, height: 60 };
const CHIP_PANEL = { x: 1040, y: 0, width: 400, height: 200 };

const organizeStorage = (width: number) => ({
  'fourfour.organize.expanded': 'true',
  'fourfour.organize.width': String(width),
});

const unanalysed = (fixture: { tracks: Record<string, unknown>[]; playlists: unknown[] }) => ({
  ...fixture,
  tracks: fixture.tracks.map((t) => ({ ...t, tempo: 0, key: '', has_analysis: false, has_cues: false })),
});

async function emitEvery(page: Page, event: (n: number) => [string, unknown], perSecond: number) {
  await page.evaluate(
    ({ source, perSecond }) => {
      const make = new Function('n', `return (${source})(n)`) as (n: number) => [string, unknown];
      const w = window as unknown as { __smokeEmit: (e: string, p: unknown) => void };
      let n = 0;
      window.setInterval(() => {
        const [name, payload] = make(n++);
        w.__smokeEmit(name, payload);
      }, 1000 / perSecond);
    },
    { source: event.toString(), perSecond },
  );
}

const statusChip: Scenario = async ({ boot, shot, emit }) => {
  let page = await boot();
  await page.waitForTimeout(600);
  await shot('1-idle-track-count', { clip: CHIP });

  await emit('analysis-progress', { current: 7, total: 20, message: 'Analyzed Smoke Track 07.mp3 (7/20)', track_id: 7 });
  await page.waitForTimeout(300);
  await shot('2-analysing', { clip: CHIP });
  await emit('analysis-complete', { analyzed: 20 });
  await page.waitForTimeout(500);

  page = await boot();
  await page.evaluate(() => {
    const w = window as unknown as { __smokeHoldImport: boolean; __smokeImport: unknown };
    w.__smokeHoldImport = true;
    w.__smokeImport = { tracks: [], already: 0 };
  });
  await page.waitForTimeout(450);
  await page.locator(chip).click();
  await page.locator(`${menu} .ff-menu__item`).filter({ hasText: 'Import folder' }).click();
  await emit('import-progress', { done: 12, total: 40 });
  await page.waitForTimeout(300);
  await shot('3-importing', { clip: CHIP });

  page = await boot();
  await page.waitForTimeout(450);
  await page.locator(chip).click();
  await page.locator(menu).waitFor();
  await shot('4-panel-open-idle', { clip: CHIP_PANEL });
};

const sidebar: Scenario = async ({ boot, shot }) => {
  let page = await boot();
  await page.waitForTimeout(300);
  await shot('1-closed');

  page = await boot({ storage: organizeStorage(420) });
  await page.locator('.ff-organize--expanded').waitFor();
  await shot('2-open');

  page = await boot({ storage: organizeStorage(420) });
  const handle = (await page.locator('.ff-organize__resize-handle').first().boundingBox())!;
  await page.mouse.move(handle.x + 2, handle.y + 100);
  await page.mouse.down();
  await page.mouse.move(2, handle.y + 100, { steps: 8 });
  await page.mouse.up();
  await page.waitForTimeout(300);
  await shot('3-open-max-width');
};

const skeleton: Scenario = async ({ boot, shot }) => {
  const page = await boot({ holdLoad: true });
  await page.locator('[data-table-skeleton]').waitFor();
  await page.waitForTimeout(400);
  await shot('1-skeleton-rows-on-start');
  await page.evaluate(() => (window as unknown as { __smokeReleaseLoad?: () => void }).__smokeReleaseLoad?.());
  await page.locator('[data-track-row]').first().waitFor();
  await page.waitForTimeout(300);
  await shot('2-loaded');
};

const scrollWhileAnalysing: Scenario = async ({ boot, finishVideo }) => {
  const page = await boot({ video: true, state: unanalysed });
  await page.waitForTimeout(800);
  await page.locator('.ff-colh__cell[data-col-key="bpm"] .ff-colh__label').click();
  await page.waitForTimeout(400);
  await emitEvery(page, (n) => ['analysis-progress', { current: (n % 150) + 1, total: 150, message: `Analyzed Smoke Track ${(n % 150) + 1}.mp3`, track_id: ((n * 7919) % 150) + 1 }], 6);
  await page.mouse.move(700, 500);
  for (let i = 0; i < 24; i++) {
    await page.mouse.wheel(0, i < 12 ? 260 : -260);
    await page.waitForTimeout(250);
  }
  await finishVideo('scroll-while-analysing', { gif: false });
};

export default {
  'ov1028-status-chip': statusChip,
  'ov1028-sidebar': sidebar,
  'ov1028-skeleton': skeleton,
  'ov1028-scroll-analysing': scrollWhileAnalysing,
} satisfies Record<string, Scenario>;
