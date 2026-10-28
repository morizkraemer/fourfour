import { writeFileSync } from 'node:fs';
import type { Page } from '@playwright/test';
import { expect, rows, sidebarPlaylist, test } from './app.ts';
import { fixtureAnalysis, fixtureState } from './fixture.ts';
import { installTauriMock } from './tauri-mock.ts';

const TRACKS = Number(process.env.FF_PERF_TRACKS ?? 0);
const REPORT = process.env.FF_PERF_OUT;

interface Probe {
  longTasks: { start: number; duration: number }[];
  t0: number;
}

const results: Record<string, number> = {};
const record = (name: string, value: number) => {
  results[name] = Math.round(value * 10) / 10;
};
const median = (values: number[]) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
const percentile = (values: number[], p: number) => [...values].sort((a, b) => a - b)[Math.min(values.length - 1, Math.floor((p / 100) * values.length))];

async function installProbes(page: Page) {
  await page.evaluate(() => {
    const w = window as unknown as { __perf: Probe };
    w.__perf = { longTasks: [], t0: 0 };
    new PerformanceObserver((list) => {
      for (const e of list.getEntries()) w.__perf.longTasks.push({ start: e.startTime, duration: e.duration });
    }).observe({ type: 'longtask' });
    for (const type of ['pointerdown', 'keydown']) {
      document.addEventListener(type, () => (w.__perf.t0 = performance.now()), true);
    }
  });
}

const settle = (page: Page) =>
  page.evaluate(
    () =>
      new Promise<number>((resolve) =>
        setTimeout(
          () => requestAnimationFrame(() => requestAnimationFrame(() => resolve(performance.now() - (window as unknown as { __perf: Probe }).__perf.t0))),
          0,
        ),
      ),
  );

const blockingSince = (page: Page, since: number) =>
  page.evaluate((s) => (window as unknown as { __perf: Probe }).__perf.longTasks.filter((t) => t.start >= s).reduce((a, t) => a + t.duration, 0), since);

const now = (page: Page) => page.evaluate(() => performance.now());

type Profile = { nodes: { id: number; children?: number[]; callFrame: { functionName: string; url: string; lineNumber: number } }[]; samples: number[]; timeDeltas: number[] };

async function profiled(page: Page, name: string, run: () => Promise<void>) {
  if (process.env.FF_PERF_PROFILE !== name) return run();
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Profiler.enable');
  await cdp.send('Profiler.start');
  await run();
  const { profile } = (await cdp.send('Profiler.stop')) as { profile: Profile };
  printProfile(name, profile);
  await cdp.detach();
}

function printProfile(name: string, profile: Profile) {
  const selfMs = new Map<number, number>();
  profile.samples.forEach((id, i) => selfMs.set(id, (selfMs.get(id) ?? 0) + profile.timeDeltas[i] / 1000));
  const byId = new Map(profile.nodes.map((n) => [n.id, n]));
  const totalMs = new Map<number, number>();
  const total = (id: number): number => {
    const cached = totalMs.get(id);
    if (cached !== undefined) return cached;
    const node = byId.get(id)!;
    const sum = (selfMs.get(id) ?? 0) + (node.children ?? []).reduce((a, child) => a + total(child), 0);
    totalMs.set(id, sum);
    return sum;
  };
  const label = (n: { callFrame: { functionName: string; url: string; lineNumber: number } }) => `${n.callFrame.functionName || '(anon)'} ${n.callFrame.url.split('/').pop()}:${n.callFrame.lineNumber}`;
  const rank = (ms: (id: number) => number) =>
    profile.nodes
      .filter((n) => n.callFrame.url.includes('/assets/') || !n.callFrame.url)
      .map((n) => ({ ms: ms(n.id), text: label(n) }))
      .sort((x, y) => y.ms - x.ms)
      .slice(0, 14);
  const fmt = (rows: { ms: number; text: string }[]) => rows.map((r) => `${r.ms.toFixed(1).padStart(8)} ms  ${r.text}`).join('\n');
  console.log(`profile ${name} self\n${fmt(rank((id) => selfMs.get(id) ?? 0))}\nprofile ${name} inclusive\n${fmt(rank(total))}`);
}

async function cdpMetrics(page: Page) {
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Performance.enable');
  const read = async () => Object.fromEntries(((await cdp.send('Performance.getMetrics')) as { metrics: { name: string; value: number }[] }).metrics.map((m) => [m.name, m.value * 1000]));
  return read;
}

let readMetrics: (() => Promise<Record<string, number>>) | null = null;

async function timed(page: Page, name: string, action: () => Promise<void>) {
  readMetrics ??= await cdpMetrics(page);
  const since = await now(page);
  const before = await readMetrics();
  await profiled(page, name, async () => {
    await action();
    record(`${name} ms`, await settle(page));
  });
  const after = await readMetrics();
  record(`${name} long-task ms`, await blockingSince(page, since));
  record(`${name} script ms`, after.ScriptDuration - before.ScriptDuration);
  record(`${name} layout+style ms`, after.LayoutDuration - before.LayoutDuration + after.RecalcStyleDuration - before.RecalcStyleDuration);
  record(`${name} main-thread task ms`, after.TaskDuration - before.TaskDuration);
}

function scrollFrames(page: Page, frames: number, step: number) {
  return page.evaluate(
    ({ frames, step }) =>
      new Promise<number[]>((resolve) => {
        const el = document.querySelector('.ff-table__scroll') as HTMLElement;
        el.scrollTop = 0;
        const deltas: number[] = [];
        let last = performance.now();
        let n = 0;
        const tick = () => {
          const t = performance.now();
          deltas.push(t - last);
          last = t;
          el.scrollTop += step;
          if (++n < frames) requestAnimationFrame(tick);
          else resolve(deltas);
        };
        requestAnimationFrame(tick);
      }),
    { frames, step },
  );
}

test.describe('perf at scale', () => {
  test.skip(TRACKS === 0, 'set FF_PERF_TRACKS=10000 to run');
  test.setTimeout(300_000);

  test('measure', async ({ smoke: { page } }) => {
    const boot = await page.evaluate(() => ({
      readyAt: performance.now(),
      domNodes: document.getElementsByTagName('*').length,
      rowsInDom: document.querySelectorAll('[data-track-row]').length,
    }));
    record('boot first row at ms', boot.readyAt);
    record('boot dom nodes', boot.domNodes);
    record('boot rows in dom', boot.rowsInDom);
    await page.waitForTimeout(1500);
    await installProbes(page);

    const steady = await scrollFrames(page, 120, 90);
    record('scroll frame p50 ms', median(steady));
    record('scroll frame p95 ms', percentile(steady, 95));
    record('scroll frame max ms', Math.max(...steady));
    const fling = await scrollFrames(page, 30, 9000);
    record('fling frame p95 ms', percentile(fling, 95));
    record('fling frame max ms', Math.max(...fling));
    await page.evaluate(() => ((document.querySelector('.ff-table__scroll') as HTMLElement).scrollTop = 0));

    const search = page.locator('.ff-gheader__search input');
    await search.click();
    const keys: number[] = [];
    await profiled(page, 'search typing', async () => {
      for (const ch of 'smoke track 12') {
        await page.keyboard.press(ch === ' ' ? 'Space' : ch);
        keys.push(await settle(page));
      }
    });
    record('search keystroke first ms', keys[0]);
    record('search keystroke median ms', median(keys));
    record('search keystroke worst ms', Math.max(...keys));
    await timed(page, 'search clear', () => search.fill(''));
    await page.keyboard.press('Escape');
    await expect(page.locator('.ff-cp')).toHaveCount(0);
    await expect(rows(page).first()).toBeVisible();

    const titleHeader = page.locator('.ff-colh__cell[data-col-key="title"] .ff-colh__label');
    await timed(page, 'sort title asc', () => titleHeader.click());
    await timed(page, 'sort title desc', () => titleHeader.click());
    await timed(page, 'sort artist', () => page.locator('.ff-colh__cell[data-col-key="artist"] .ff-colh__label').click());

    const selects: number[] = [];
    const analysed: number[] = [];
    for (const index of [3, 5, 7, 9, 11]) {
      const result = await page.evaluate(
        (i) =>
          new Promise<{ title: number; markers: number }>((resolve) => {
            const row = document.querySelectorAll('[data-track-row]')[i] as HTMLElement;
            const wanted = row.querySelector('.ff-trow__cell--title .ff-trow__text')!.textContent!.trim();
            const t0 = performance.now();
            let title = 0;
            row.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
            const poll = () => {
              const input = document.querySelector('.ff-inspector input[aria-label="Title"]') as HTMLInputElement | null;
              if (!title && input?.value === wanted) title = performance.now() - t0;
              if (title && document.querySelector('.ff-inspector [data-marker-key="h1"]')) return resolve({ title, markers: performance.now() - t0 });
              if (performance.now() - t0 > 5000) return resolve({ title, markers: -1 });
              requestAnimationFrame(poll);
            };
            requestAnimationFrame(poll);
          }),
        index,
      );
      selects.push(result.title);
      analysed.push(result.markers);
      await page.waitForTimeout(300);
    }
    record('select to inspector analysis median ms', median(analysed));
    record('select to inspector median ms', median(selects));
    record('select to inspector worst ms', Math.max(...selects));

    await timed(page, 'open 2k playlist', async () => {
      await sidebarPlaylist(page, 'Big Playlist').click();
      await expect(page.locator('.ff-gheader__count')).toHaveText(Math.min(2000, TRACKS).toLocaleString('en-US'));
    });
    const playlistScroll = await scrollFrames(page, 120, 90);
    record('playlist scroll frame p95 ms', percentile(playlistScroll, 95));
    record('playlist scroll frame max ms', Math.max(...playlistScroll));
    await timed(page, 'playlist sort', () => titleHeader.click());
    await page.locator('.ff-sbrow__label', { hasText: /^All Tracks$/ }).click();
    await expect(page.locator('.ff-gheader__count')).toHaveText(TRACKS.toLocaleString('en-US'));

    await timed(page, 'palette open', async () => {
      await page.keyboard.press('Control+k');
      await expect(page.locator('.ff-cp')).toBeVisible();
    });
    const paletteKeys: number[] = [];
    for (const ch of 'settings') {
      await page.keyboard.press(ch);
      paletteKeys.push(await settle(page));
    }
    record('palette filter keystroke median ms', median(paletteKeys));
    record('palette filter keystroke worst ms', Math.max(...paletteKeys));
    await page.keyboard.press('Escape');

    record(
      'save_state invokes',
      await page.evaluate(() => (window as unknown as { __smokeInvokes: { cmd: string }[] }).__smokeInvokes.filter((i) => i.cmd === 'save_state').length),
    );

    console.log(JSON.stringify(results, null, 2));
    if (REPORT) writeFileSync(REPORT, JSON.stringify(results, null, 2));
  });

  test('boot profile', async ({ page }) => {
    test.skip(process.env.FF_PERF_PROFILE !== 'boot', 'set FF_PERF_PROFILE=boot');
    await page.exposeFunction('__smokeAnalysis', (trackId: number) => fixtureAnalysis(trackId));
    await page.addInitScript(installTauriMock, fixtureState());
    const cdp = await page.context().newCDPSession(page);
    await cdp.send('Profiler.enable');
    await cdp.send('Profiler.start');
    await page.goto('/');
    await page.locator('[data-track-row]').first().waitFor();
    await page.waitForTimeout(500);
    const { profile } = (await cdp.send('Profiler.stop')) as { profile: Profile };
    printProfile('boot', profile);
  });
});
