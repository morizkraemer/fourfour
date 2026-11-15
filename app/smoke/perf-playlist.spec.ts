import { createHash } from 'node:crypto';
import { appendFileSync, readFileSync, writeFileSync } from 'node:fs';
import { expect, test, type Locator, type Page } from '@playwright/test';
import type { LoadedState } from '../src/services/tauri.svelte.ts';
import { decodeTrackTable, isTrackTable } from '../src/services/track-table.ts';
import { mockAnalysisData } from '../src/mocks/analysis.ts';
import { sidebarPlaylist } from './app.ts';
import type { SmokeWindow } from './tauri-mock.ts';
import { installTauriMock } from './tauri-mock.ts';

const STATE = process.env.FF_PERF_STATE;
const REPORT = process.env.FF_PERF_OUT;
const RUNS = Number(process.env.FF_PERF_RUNS ?? 1);
const LABEL = process.env.FF_PERF_LABEL ?? '';
const LISTS = (process.env.FF_PERF_PLAYLISTS ?? 'R124 1k,R124 5k,R124 20k').split(',');
const COLUMN_SWEEP = process.env.FF_PERF_SCENARIO === 'columns';
const SORTABLE = ['status', 'fav', 'index', 'title', 'artist', 'album', 'label', 'genre', 'year', 'rating', 'color', 'bpm', 'key', 'time'];

interface Step {
  name: string;
  total: number;
  script: number;
  task: number;
  layout: number;
  js: number;
  view: string;
}

function state(): LoadedState {
  const dumped = JSON.parse(readFileSync(STATE!, 'utf8'));
  return { ...dumped, tracks: isTrackTable(dumped.tracks) ? decodeTrackTable(dumped.tracks) : dumped.tracks } as LoadedState;
}

async function metrics(page: Page) {
  if (process.env.FF_SMOKE_BROWSER === 'webkit') return async () => ({ script: 0, task: 0, layout: 0 });
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Performance.enable');
  return async () => {
    const m = Object.fromEntries(((await cdp.send('Performance.getMetrics')) as { metrics: { name: string; value: number }[] }).metrics.map((x) => [x.name, x.value * 1000]));
    return { script: m.ScriptDuration, task: m.TaskDuration, layout: m.LayoutDuration + m.RecalcStyleDuration };
  };
}

const settle = (page: Page) =>
  page.evaluate(
    () =>
      new Promise<number>((resolve) => {
        const t0 = (window as unknown as { __inputAt: number }).__inputAt;
        setTimeout(() => requestAnimationFrame(() => requestAnimationFrame(() => resolve(performance.now() - t0))), 0);
      }),
  );

const jsDone = (page: Page) => page.evaluate(() => (window as unknown as { __jsAt: number; __inputAt: number }).__jsAt - (window as unknown as { __jsAt: number; __inputAt: number }).__inputAt);

const stamp = (page: Page) =>
  page.evaluate(() => {
    const w = window as unknown as { __inputAt: number; __jsAt: number };
    w.__inputAt = performance.now();
    setTimeout(() => (w.__jsAt = performance.now()), 0);
  });

const topRows = (page: Page) =>
  page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>('[data-track-row]')]
      .map((row) => `${row.dataset.rowIndex}:${row.textContent?.replace(/\s+/g, ' ').trim()}`)
      .join('\n'),
  );

const sha = (text: string) => createHash('sha1').update(text).digest('hex').slice(0, 12);

async function fullView(page: Page) {
  const count = (await page.locator('.ff-gheader__count').textContent())?.trim() ?? '';
  const parts: string[] = [count];
  const heights = await page.evaluate(() => {
    const el = document.querySelector('.ff-table__scroll') as HTMLElement;
    return el.scrollHeight - el.clientHeight;
  });
  for (const at of [0, 0.5, 1]) {
    await page.evaluate((top) => ((document.querySelector('.ff-table__scroll') as HTMLElement).scrollTop = top), Math.floor(heights * at));
    await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
    await page.waitForTimeout(60);
    parts.push(await topRows(page));
  }
  await page.evaluate(() => ((document.querySelector('.ff-table__scroll') as HTMLElement).scrollTop = 0));
  await page.waitForTimeout(350);
  return `${count}/${sha(parts.join('\n--\n'))}`;
}

const rowLocator = (page: Page) => page.locator('[data-track-row]');

async function rowAt(page: Page, index: number): Promise<Locator> {
  const row = page.locator(`[data-track-row][data-row-index="${index}"]`);
  await row.waitFor();
  return row;
}

type Profile = { nodes: { id: number; children?: number[]; callFrame: { functionName: string; url: string; lineNumber: number } }[]; samples: number[]; timeDeltas: number[] };

async function startProfile(page: Page, out: string) {
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Profiler.enable');
  await cdp.send('Profiler.setSamplingInterval', { interval: 100 });
  await cdp.send('Profiler.start');
  return async () => {
    const { profile } = (await cdp.send('Profiler.stop')) as { profile: Profile };
    writeFileSync(out, JSON.stringify(profile));
    await cdp.detach();
  };
}

async function runScenario(page: Page, read: () => Promise<{ script: number; task: number; layout: number }>, steps: Step[]) {
  const search = page.locator('.ff-gheader__search input');
  const measure = async (name: string, action: () => Promise<void>, options: { stamped?: boolean; gap?: number } = {}) => {
    const before = await read();
    if (options.stamped) await stamp(page);
    await action();
    const total = await settle(page);
    const js = await jsDone(page);
    const after = await read();
    const view = sha(await topRows(page));
    steps.push({ name, total, script: after.script - before.script, task: after.task - before.task, layout: after.layout - before.layout, js, view });
    await page.waitForTimeout(options.gap ?? 150);
  };
  const checkpoint = async (name: string) => {
    steps.push({ name: `VIEW ${name}`, total: 0, script: 0, task: 0, layout: 0, js: 0, view: await fullView(page) });
  };
  const open = (list: string, label = `switch ${list}`) => measure(label, () => sidebarPlaylist(page, list).click(), { gap: 400 });
  const openAll = () => measure('switch All Tracks', () => page.locator('.ff-sbrow__label', { hasText: /^All Tracks$/ }).click(), { gap: 400 });
  const type = async (query: string, label: string) => {
    for (let i = 0; i < query.length; i++) await measure(`${label} key ${i + 1} '${query[i]}'`, () => page.keyboard.press(query[i]));
  };
  const sortBy = (key: string, name: string) => measure(`sort ${name}`, () => page.locator(`.ff-colh__cell[data-col-key="${key}"] .ff-colh__label`).click(), { gap: 400 });
  const clearSortFromMenu = async () => {
    await sidebarPlaylist(page, LISTS[0]).click();
    await page.waitForTimeout(500);
    await page.locator('.ff-colh__cell[data-col-key="title"]').click({ button: 'right' });
    await page.getByText('Clear sort', { exact: true }).click();
    await page.waitForTimeout(500);
  };
  const clearSearch = async () => {
    await search.fill('');
    await page.keyboard.press('Escape');
    await page.waitForTimeout(300);
  };

  for (const list of LISTS) {
    await open(list);
    await checkpoint(`${list} opened`);
    await search.click();
    await type('house', `${list} type`);
    await checkpoint(`${list} house`);
    for (let i = 0; i < 2; i++) await measure(`${list} backspace ${i + 1}`, () => page.keyboard.press('Backspace'));
    await measure(`${list} clear search`, () => search.fill(''), { gap: 400 });
    await page.keyboard.press('Escape');
    await openAll();
  }

  for (const list of LISTS) {
    await open(list, `switch ${list} sorted`);
    await sortBy('title', `${list} title asc`);
    await checkpoint(`${list} title asc`);
    await sortBy('title', `${list} title desc`);
    await checkpoint(`${list} title desc`);
    await sortBy('artist', `${list} artist asc`);
    await checkpoint(`${list} artist asc`);
    await sortBy('bpm', `${list} bpm asc`);
    await checkpoint(`${list} bpm asc`);
    await sortBy('key', `${list} key asc`);
    await checkpoint(`${list} key asc`);
    await search.click();
    await type('a', `${list} sorted type`);
    await measure(`${list} sorted clear search`, () => search.fill(''), { gap: 400 });
    await type('house', `${list} sorted type`);
    await checkpoint(`${list} sorted house`);
    for (let i = 0; i < 2; i++) await measure(`${list} sorted backspace ${i + 1}`, () => page.keyboard.press('Backspace'));
    await measure(`${list} sorted clear search`, () => search.fill(''), { gap: 400 });
    await page.keyboard.press('Escape');
    await openAll();
  }
  await clearSortFromMenu();

  for (const list of LISTS.slice(-2)) {
    await openAll();
    await (await rowAt(page, 5)).click();
    await (await rowAt(page, 14)).click({ modifiers: ['Shift'] });
    const added = await (await rowAt(page, 9)).locator('.ff-trow__cell--title').boundingBox();
    const sidebarRow = sidebarPlaylist(page, list);
    await sidebarRow.scrollIntoViewIfNeeded();
    const into = (await sidebarRow.boundingBox())!;
    await page.mouse.move(added!.x + 20, added!.y + added!.height / 2);
    await page.mouse.down();
    await page.mouse.move(added!.x + 30, added!.y + added!.height / 2 + 12, { steps: 4 });
    await page.mouse.move(into.x + into.width / 2, into.y + into.height / 2, { steps: 12 });
    await page.waitForTimeout(200);
    await measure(`${list} add 10 rows`, () => page.mouse.up(), { stamped: true, gap: 500 });
    await open(list);
    await checkpoint(`${list} after add`);
    await rowAt(page, 20);
    await (await rowAt(page, 10)).click();
    await (await rowAt(page, 19)).click({ modifiers: ['Shift'] });
    const first = await (await rowAt(page, 12)).locator('.ff-trow__cell--title').boundingBox();
    const target = await (await rowAt(page, 2)).locator('.ff-trow__cell--title').boundingBox();
    await page.mouse.move(first!.x + 20, first!.y + first!.height / 2);
    await page.mouse.down();
    await page.mouse.move(first!.x + 30, first!.y + first!.height / 2 - 12, { steps: 4 });
    await page.mouse.move(target!.x + 20, target!.y + target!.height / 2, { steps: 10 });
    await page.waitForTimeout(200);
    await measure(`${list} reorder 10 rows`, () => page.mouse.up(), { stamped: true, gap: 500 });
    await checkpoint(`${list} reordered`);

    await (await rowAt(page, 30)).click();
    await (await rowAt(page, 39)).click({ modifiers: ['Shift'] });
    await (await rowAt(page, 39)).click({ button: 'right' });
    await measure(`${list} remove 10 rows`, () => page.getByText(`Remove from ${list}`, { exact: true }).click(), { gap: 500 });
    await checkpoint(`${list} after remove`);
    await openAll();
  }
}

async function runColumnSweep(page: Page, read: () => Promise<{ script: number; task: number; layout: number }>, steps: Step[]) {
  const measure = async (name: string, action: () => Promise<void>, gap = 400) => {
    const before = await read();
    await action();
    const total = await settle(page);
    const js = await jsDone(page);
    const after = await read();
    steps.push({ name, total, script: after.script - before.script, task: after.task - before.task, layout: after.layout - before.layout, js, view: sha(await topRows(page)) });
    await page.waitForTimeout(gap);
  };
  const checkpoint = async (name: string) => {
    steps.push({ name: `VIEW ${name}`, total: 0, script: 0, task: 0, layout: 0, js: 0, view: await fullView(page) });
  };
  for (const list of LISTS) {
    await measure(`switch ${list}`, () => sidebarPlaylist(page, list).click());
    await checkpoint(`${list} opened`);
    for (const key of SORTABLE) {
      for (const dir of ['asc', 'desc']) {
        await measure(`sort ${list} ${key} ${dir}`, () => page.locator(`.ff-colh__cell[data-col-key="${key}"]`).click({ position: { x: 6, y: 8 } }));
        await checkpoint(`${list} ${key} ${dir}`);
      }
    }
    await measure(`switch All Tracks after ${list}`, () => page.locator('.ff-sbrow__label', { hasText: /^All Tracks$/ }).click());
  }
}

test.describe('playlist view latency', () => {
  test.skip(!STATE, 'set FF_PERF_STATE=/path/state.json (r124-playlist-variant.py of a bench_startup dump)');
  test.setTimeout(900_000);

  for (let run = 0; run < RUNS; run++) {
    test(`run ${run}`, async ({ page }) => {
      await page.exposeFunction('__smokeAnalysis', (trackId: number) => mockAnalysisData(trackId, 180 + ((trackId * 7) % 300), 12400, '8A'));
      await page.addInitScript(installTauriMock, state());
      if (COLUMN_SWEEP) {
        await page.addInitScript((keys: string[]) => {
          const columns = [
            ...keys.map((key) => ({ key, label: key.toUpperCase(), width: key === 'title' || key === 'artist' ? 140 : 64 })),
            { key: 'wave', hidden: true },
            { key: 'cover', hidden: true },
          ];
          localStorage.setItem('fourfour.tableColumns.v3', JSON.stringify({ layouts: { playlist: { columns, sortKey: null, sortDir: 'asc' } } }));
        }, SORTABLE);
      }
      await page.addInitScript(() => {
        const internals = (window as unknown as { __TAURI_INTERNALS__: { invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> } }).__TAURI_INTERNALS__;
        const invoke = internals.invoke.bind(internals);
        internals.invoke = (cmd, args) => {
          if (cmd === 'save_state') JSON.stringify(args);
          return invoke(cmd, args);
        };
      });
      await page.goto('/');
      await rowLocator(page).first().waitFor({ timeout: 120_000 });
      await page.waitForTimeout(6000);
      await page.evaluate(() => {
        const w = window as unknown as { __inputAt: number; __jsAt: number };
        const mark = () => {
          w.__inputAt = performance.now();
          setTimeout(() => (w.__jsAt = performance.now()), 0);
        };
        for (const type of ['keydown', 'pointerdown']) document.addEventListener(type, mark, true);
        const smoke = window as unknown as SmokeWindow;
        smoke.__smokeAnalysis = (trackId) => (smoke.__smokeAnalysis as never) && Promise.resolve({ track_id: trackId });
      });
      const read = await metrics(page);
      const steps: Step[] = [];
      const stopProfile = process.env.FF_PERF_PROFILE_JSON ? await startProfile(page, process.env.FF_PERF_PROFILE_JSON) : null;
      await (COLUMN_SWEEP ? runColumnSweep : runScenario)(page, read, steps);
      await stopProfile?.();
      expect(steps.length).toBeGreaterThan(0);
      const line = JSON.stringify({ label: LABEL, run, steps });
      if (REPORT) appendFileSync(REPORT, line + '\n');
      else console.log(line);
    });
  }
});
