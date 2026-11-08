import type { CDPSession, Locator, Page } from '@playwright/test';
import { expect, loadByDoubleClick, rowByTitle, test, type Smoke } from './app.ts';
import { loadFactor } from './load-factor.ts';
import type { SmokeWindow } from './tauri-mock.ts';

const CYCLES = 6;
const WARMUP_CYCLES = 2;
const STABLE_READS = 3;
const LISTENER_TOLERANCE = 2;
const NODE_TOLERANCE = 40;
const RECHECKS = 4;
const CDP_COUNTERS = process.env.FF_SMOKE_BROWSER !== 'webkit';

interface Counters {
  listeners: number;
  nodes: number;
  callbacks: number;
}

async function counters(page: Page, cdp: CDPSession | null): Promise<Counters> {
  const callbacks = await page.evaluate(() => Object.keys(window).filter((key) => /^_\d+$/.test(key)).length);
  if (!cdp) return { listeners: 0, nodes: 0, callbacks };
  await cdp.send('HeapProfiler.collectGarbage');
  const dom = (await cdp.send('Memory.getDOMCounters')) as { nodes: number; jsEventListeners: number };
  return { listeners: dom.jsEventListeners, nodes: dom.nodes, callbacks };
}

function sameCounters(a: Counters, b: Counters): boolean {
  return a.listeners === b.listeners && a.nodes === b.nodes && a.callbacks === b.callbacks;
}

async function settledCounters(page: Page, cdp: CDPSession | null): Promise<Counters> {
  const minimumWindowMs = 150 * loadFactor();
  let previous = await counters(page, cdp);
  let stableSince = Date.now();
  let stableReads = 1;
  for (let attempt = 0; attempt < 80; attempt++) {
    await page.evaluate(
      () =>
        new Promise<void>((resolve) =>
          requestAnimationFrame(() =>
            requestAnimationFrame(() =>
              typeof requestIdleCallback === 'function'
                ? requestIdleCallback(() => resolve(), { timeout: 1000 })
                : setTimeout(resolve, 50),
            ),
          ),
        ),
    );
    const current = await counters(page, cdp);
    if (sameCounters(current, previous)) {
      stableReads++;
      if (stableReads >= STABLE_READS && Date.now() - stableSince >= minimumWindowMs) return current;
    } else {
      stableReads = 1;
      stableSince = Date.now();
    }
    previous = current;
  }
  return previous;
}

function growthViolations(before: Counters, after: Counters, cdp: CDPSession | null): string[] {
  const violations: string[] = [];
  if (after.callbacks - before.callbacks !== 0) violations.push('registered event callbacks');
  if (!cdp) return violations;
  if (after.listeners - before.listeners > LISTENER_TOLERANCE) violations.push('event listeners');
  if (after.nodes - before.nodes > NODE_TOLERANCE) violations.push('DOM nodes, attached or detached');
  return violations;
}

async function measureGrowth(page: Page, cdp: CDPSession | null, cycle: () => Promise<void>) {
  for (let i = 0; i < WARMUP_CYCLES; i++) await cycle();
  const before = await settledCounters(page, cdp);
  for (let i = 0; i < CYCLES; i++) await cycle();
  let after = await settledCounters(page, cdp);
  for (let recheck = 0; recheck < RECHECKS && growthViolations(before, after, cdp).length > 0; recheck++) {
    after = await settledCounters(page, cdp);
  }
  return { before, after, violations: growthViolations(before, after, cdp) };
}

async function openAndClosePalette(page: Page) {
  await page.keyboard.press('Control+k');
  await expect(page.locator('.ff-cp')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.locator('.ff-cp')).toHaveCount(0);
}

async function openAndCloseSettings(page: Page) {
  await page.keyboard.press('Control+,');
  const items = page.locator('.ff-settings-sheet__nav-item');
  await expect(items.first()).toBeVisible();
  for (let i = 0; i < Math.min(3, await items.count()); i++) await items.nth(i).click();
  await page.keyboard.press('Escape');
  await expect(items.first()).toHaveCount(0);
}

async function expandAndCollapsePlayer(page: Page) {
  await page.locator('.ff-player-compact__expand-btn').click();
  await expect(page.locator('.ff-xp .waveform-display__zoom')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.locator('.ff-xp')).toHaveCount(0);
}

const title = CDP_COUNTERS
  ? 'opening and closing the palette, settings and the expanded player leaves no listeners, nodes or callbacks behind'
  : 'opening and closing the palette, settings and the expanded player leaves no callbacks behind (listener and node counters are Chromium-only)';

test(title, async ({ smoke }) => {
  test.setTimeout(180_000 * loadFactor());
  const { page } = smoke;
  await page.evaluate(() => ((window as unknown as SmokeWindow).__smokeDetail = 'serve'));
  await loadByDoubleClick(page, 'Smoke Track 03');
  const cdp = CDP_COUNTERS ? await page.context().newCDPSession(page) : null;
  await cdp?.send('Memory.getDOMCounters');

  const cycle = async () => {
    await openAndClosePalette(page);
    await openAndCloseSettings(page);
    await expandAndCollapsePlayer(page);
  };
  const { before, after, violations } = await measureGrowth(page, cdp, cycle);
  expect(violations, `growth after ${CYCLES} cycles, before ${JSON.stringify(before)} after ${JSON.stringify(after)}`).toEqual([]);
});

interface PinnedGetterProbe {
  getters: (() => unknown)[];
}

async function recordCurrentTargetGetters(page: Page) {
  await page.addInitScript(() => {
    const probe: PinnedGetterProbe = { getters: [] };
    (window as unknown as { __pinnedGetterProbe: PinnedGetterProbe }).__pinnedGetterProbe = probe;
    const define = Object.defineProperty;
    Object.defineProperty = function (target: object, key: PropertyKey, descriptor: PropertyDescriptor) {
      if (target instanceof Event && key === 'currentTarget' && descriptor.get) probe.getters.push(descriptor.get);
      return define(target, key, descriptor);
    } as typeof Object.defineProperty;
  });
  await page.reload();
  await page.locator('[data-track-row]').first().waitFor();
}

const detachedCurrentTargets = (page: Page) =>
  page.evaluate(() =>
    (window as unknown as { __pinnedGetterProbe: PinnedGetterProbe }).__pinnedGetterProbe.getters
      .map((get) => get())
      .filter((node): node is Element => node instanceof Element && !node.isConnected)
      .map((node) => `${node.tagName.toLowerCase()}.${node.className}`),
  );

const TRACK = 'Smoke Track 03';
const SANDISK = '/Volumes/SanDisk 32GB';

interface PinningScenario {
  prepare?: (page: Page) => Promise<void>;
  run: (smoke: Smoke) => Promise<void>;
}

const expandPlayer = async (page: Page) => {
  await loadByDoubleClick(page, TRACK);
  await page.locator('.ff-player-compact__expand-btn').click();
  await expect(page.locator('.ff-xp')).toBeVisible();
};

const press = async (page: Page, locator: Locator) => {
  const box = (await locator.boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.up();
};

const pinning: Record<string, PinningScenario> = {
  'settings sheet closed with Escape': {
    run: async ({ page }) => {
      await page.keyboard.press('Control+,');
      const items = page.locator('.ff-settings-sheet__nav-item');
      await expect(items.first()).toBeVisible();
      await items.nth(1).click();
      await page.keyboard.press('Escape');
      await expect(items.first()).toHaveCount(0);
    },
  },
  'command palette closed with Escape': {
    run: async ({ page }) => {
      await page.keyboard.press('Control+k');
      await expect(page.locator('.ff-cp')).toBeVisible();
      await page.keyboard.press('Escape');
      await expect(page.locator('.ff-cp')).toHaveCount(0);
    },
  },
  'command palette setting choice clicked, then closed': {
    run: async ({ page }) => {
      await page.keyboard.press('Control+k');
      await page.keyboard.type('Analysis engine');
      await page.locator('.ff-cp__choice').first().click();
      await page.mouse.click(4, 4);
      await expect(page.locator('.ff-cp')).toHaveCount(0);
    },
  },
  'row context menu clicked inside, then closed with Escape': {
    run: async ({ page }) => {
      await rowByTitle(page, TRACK).click({ button: 'right' });
      const menu = page.locator('.ff-context-menu');
      await menu.click({ position: { x: 1, y: 1 } });
      await page.keyboard.press('Escape');
      await expect(menu).toHaveCount(0);
    },
  },
  'context menu submenu clicked inside': {
    run: async ({ page }) => {
      await rowByTitle(page, TRACK).click({ button: 'right' });
      const parent = page.locator('.ff-context-menu .ff-menu__item--submenu').first();
      await parent.hover();
      const submenu = page.locator('.ff-menu--submenu');
      await submenu.click({ position: { x: 1, y: 1 } });
      await page.keyboard.press('Escape');
      await expect(page.locator('.ff-context-menu')).toHaveCount(0);
    },
  },
  'dialog clicked inside, then closed': {
    prepare: async (page) => {
      await page.addInitScript(() => {
        (window as unknown as { __smokeMissing: number[] }).__smokeMissing = [3];
      });
    },
    run: async ({ page }) => {
      await page.locator('[data-library-status]').click();
      await page.locator('.ff-context-menu .ff-menu__item').filter({ hasText: 'Library check results…' }).click();
      await expect(page.locator('[data-health-dialog]')).toBeVisible();
      await page.locator('.ff-dialog__title').click();
      await page.locator('.ff-dialog').getByRole('button', { name: 'Done' }).click();
      await expect(page.locator('[data-health-dialog]')).toHaveCount(0);
    },
  },
  'sidebar rename input clicked, then committed': {
    run: async ({ page }) => {
      await page.keyboard.press('Control+Shift+N');
      const rename = page.locator('.ff-sbrow__label-input');
      await expect(rename).toBeVisible();
      await rename.click();
      await rename.dblclick();
      await rename.fill('Leak List');
      await rename.press('Enter');
      await expect(rename).toHaveCount(0);
    },
  },
  'sidebar add button and open-in-panel button clicked': {
    run: async ({ page }) => {
      await page.locator('[aria-label="New playlist"]').click();
      const rename = page.locator('.ff-sbrow__label-input');
      await expect(rename).toBeVisible();
      await rename.press('Enter');
      await expect(rename).toHaveCount(0);
      const openInPanel = page.locator('.ff-sbrow__action').first();
      await openInPanel.locator('xpath=..').hover();
      await openInPanel.click();
      await expect(page.locator('.ff-organize--expanded')).toBeVisible();
      const handle = page.locator('.ff-organize__resize-handle');
      await press(page, handle);
      await page.locator('.ff-organize--expanded').click();
      await page.keyboard.press('Escape');
      await page.keyboard.press('Shift+Tab');
      await expect(page.locator('.ff-organize--expanded')).toHaveCount(0);
    },
  },
  'compact player buttons pressed, then expanded': {
    run: async ({ page }) => {
      await loadByDoubleClick(page, TRACK);
      await press(page, page.locator('.ff-player-compact__cue-btn'));
      await page.locator('.ff-player-compact__play-btn').click();
      await page.locator('.ff-player-compact__expand-btn').click();
      await expect(page.locator('.ff-xp')).toBeVisible();
      await expect(page.locator('.ff-player-compact__expand-btn')).toHaveCount(0);
    },
  },
  'expanded player cue pressed, tempo field edited, then collapsed': {
    run: async ({ page }) => {
      await expandPlayer(page);
      await press(page, page.locator('.ff-xp__cue-btn'));
      await page.locator('.ff-ptempo--xp .ff-tfield__value').nth(1).click();
      await expect(page.locator('.ff-tfield__input')).toBeVisible();
      await page.keyboard.press('Escape');
      await expect(page.locator('.ff-tfield__input')).toHaveCount(0);
      await page.keyboard.press('Escape');
      await expect(page.locator('.ff-xp')).toHaveCount(0);
    },
  },
  'hot cue editor typed in, then collapsed': {
    run: async ({ page }) => {
      await expandPlayer(page);
      await page.keyboard.press('Shift+C');
      await page.locator('.ff-xp__pad--set').first().click({ button: 'right' });
      const name = page.locator('.ff-cue-editor__name');
      await expect(name).toBeVisible();
      await name.fill('Drop');
      await name.press('Enter');
      await expect(name).toHaveCount(0);
      await page.keyboard.press('Escape');
      await expect(page.locator('.ff-xp')).toHaveCount(0);
    },
  },
  'inspector fields edited, then the selection changes': {
    run: async ({ page }) => {
      await rowByTitle(page, TRACK).click();
      const bpm = page.locator('.ff-inspector').getByRole('spinbutton', { name: 'BPM' });
      await bpm.dblclick();
      await page.keyboard.press('Escape');
      const title = page.locator('.ff-ghost__input').first();
      await title.click();
      await page.keyboard.press('Escape');
      await rowByTitle(page, 'Smoke Track 05').click({ modifiers: ['Shift'] });
      await expect(page.locator('.ff-trow--selected')).toHaveCount(3);
    },
  },
  'rating star clicked and column handle double-clicked, then the table is replaced by a stick view': {
    prepare: async (page) => {
      await page.evaluate(
        ([columnsKey, sticksKey]) => {
          localStorage.setItem(sticksKey, '1');
          localStorage.setItem(
            columnsKey,
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
        },
        ['fourfour.tableColumns.v3', 'smoke.sticks'],
      );
    },
    run: async ({ page }) => {
      await rowByTitle(page, TRACK).locator('.ff-trow__star').nth(2).click();
      await page.locator('.ff-colh__resize-handle').first().dblclick({ force: true });
      await page.locator(`[data-drop-stick-path="${SANDISK}"][data-drop-surface="row"] .ff-sbrow__label`).click();
      await expect(page.locator('[data-stick-view]')).toBeVisible();
      await expect(page.locator('[data-track-row]')).toHaveCount(0);
    },
  },
  'settings dropdown option picked': {
    run: async ({ page }) => {
      await page.keyboard.press('Control+,');
      await page.locator('.ff-settings-sheet__nav-item', { hasText: 'Audio' }).click();
      await page.locator('.ff-dropdown__trigger').first().click();
      await page.locator('.ff-dropdown .ff-menu__item').first().click();
      await page.keyboard.press('Escape');
      await expect(page.locator('.ff-settings-sheet__nav-item').first()).toHaveCount(0);
    },
  },
  'stick playlist removed with its x button': {
    prepare: async (page) => {
      await page.evaluate(() => {
        localStorage.setItem('smoke.sticks', '1');
        localStorage.setItem('fourfour.organize.expanded', 'false');
      });
    },
    run: async ({ page }) => {
      await page.locator(`[data-drop-stick-path="${SANDISK}"][data-drop-surface="row"] .ff-sbrow__label`).click();
      const x = page.locator('.ff-stickview__x').first();
      await x.click({ force: true });
      await expect(x).toHaveCount(0);
    },
  },
};

for (const [name, scenario] of Object.entries(pinning)) {
  test(`${name} leaves no event getter pointing into a removed element`, async ({ smoke }) => {
    await scenario.prepare?.(smoke.page);
    await recordCurrentTargetGetters(smoke.page);
    await scenario.run(smoke);
    expect(await detachedCurrentTargets(smoke.page), 'currentTarget getters that close over a detached element').toEqual([]);
  });
}

test('the growth check fails for a deliberate per-cycle leak', async ({ smoke }) => {
  test.setTimeout(180_000 * loadFactor());
  const { page } = smoke;
  const cdp = CDP_COUNTERS ? await page.context().newCDPSession(page) : null;
  await cdp?.send('Memory.getDOMCounters');

  const leakyCycle = async () => {
    await page.evaluate(() => {
      const w = window as unknown as Record<string, unknown>;
      const leaked = document.createElement('div');
      for (let i = 0; i < 10; i++) leaked.appendChild(document.createElement('span'));
      const keep = ((w.__leakedNodes as unknown[]) ??= []);
      keep.push(leaked);
      window.addEventListener('resize', () => keep.length);
      w[`_${900_000 + keep.length}`] = () => undefined;
    });
  };
  const { before, after, violations } = await measureGrowth(page, cdp, leakyCycle);
  const detail = `before ${JSON.stringify(before)} after ${JSON.stringify(after)}`;
  expect(violations, detail).toContain('registered event callbacks');
  if (cdp) {
    expect(violations, detail).toContain('event listeners');
    expect(violations, detail).toContain('DOM nodes, attached or detached');
  }
});
