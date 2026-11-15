import { createHash } from 'node:crypto';
import { appendFileSync, readFileSync, writeFileSync } from 'node:fs';
import { test, type Page } from '@playwright/test';
import type { LoadedState } from '../src/services/tauri.svelte.ts';
import { decodeTrackTable, isTrackTable } from '../src/services/track-table.ts';
import { mockAnalysisData } from '../src/mocks/analysis.ts';
import type { SmokeWindow } from './tauri-mock.ts';
import { installTauriMock } from './tauri-mock.ts';

const STATE = process.env.FF_PERF_STATE;
const REPORT = process.env.FF_PERF_OUT;
const RUNS = Number(process.env.FF_PERF_RUNS ?? 1);
const LABEL = process.env.FF_PERF_LABEL ?? '';
const FILES = Number(process.env.FF_PERF_IMPORT_FILES ?? 1000);
const FOLDERS = Number(process.env.FF_PERF_IMPORT_FOLDERS ?? 10);
const WEBKIT = process.env.FF_SMOKE_BROWSER === 'webkit';
const IDLE_MS = Number(process.env.FF_PERF_IMPORT_IDLE ?? 3000);
const ROOT = '/Downloads/Crate';

function state(): LoadedState {
  const dumped = JSON.parse(readFileSync(STATE!, 'utf8'));
  return { ...dumped, tracks: isTrackTable(dumped.tracks) ? decodeTrackTable(dumped.tracks) : dumped.tracks } as LoadedState;
}

const sha = (text: string) => createHash('sha1').update(text).digest('hex').slice(0, 12);

const visibleRows = (page: Page) =>
  page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>('[data-track-row]')]
      .map((row) => `${row.dataset.rowIndex}:${row.textContent?.replace(/\s+/g, ' ').trim()}`)
      .join('\n'),
  );

const savedPlaylists = (page: Page) =>
  page.evaluate(async () => {
    const internals = (window as unknown as { __TAURI_INTERNALS__: { invoke(cmd: string): Promise<{ playlists: { id: number; name: string; parent_id: number; is_folder: boolean; track_ids: ArrayLike<number> }[] }> } }).__TAURI_INTERNALS__;
    const { playlists } = await internals.invoke('load_state');
    return JSON.stringify(playlists.map((p) => [p.id, p.name, p.parent_id, p.is_folder, Array.from(p.track_ids ?? [])]));
  });

async function metrics(page: Page) {
  if (WEBKIT) return async () => 0;
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Performance.enable');
  return async (name = 'ScriptDuration') => {
    const m = (await cdp.send('Performance.getMetrics')) as { metrics: { name: string; value: number }[] };
    return m.metrics.find((x) => x.name === name)!.value * 1000;
  };
}

test.describe('import flow in the page', () => {
  test.skip(!STATE, 'set FF_PERF_STATE=/path/state.json (bench_startup dump)');
  test.setTimeout(600_000);

  for (let run = 0; run < RUNS; run++) {
    test(`run ${run}`, async ({ page, browser }) => {
      await page.exposeFunction('__smokeAnalysis', (trackId: number) => mockAnalysisData(trackId, 240, 12400, '8A'));
      const initial = state();
      await page.addInitScript(installTauriMock, initial);
      await page.addInitScript(() => {
        const frame = () => requestAnimationFrame(frame);
        frame();
        const w = window as unknown as { __longTasks: { start: number; duration: number }[]; __marks: Record<string, number> };
        w.__longTasks = [];
        w.__marks = {};
        try {
          new PerformanceObserver((list) => {
            for (const e of list.getEntries()) w.__longTasks.push({ start: e.startTime, duration: e.duration });
          }).observe({ type: 'longtask' });
        } catch {}
      });
      const script = await metrics(page);
      await page.goto('/');
      await page.locator('[data-track-row]').first().waitFor({ timeout: 120_000 });
      await page.waitForTimeout(6000);

      const template = initial.tracks[0];
      const tracks = Array.from({ length: FILES }, (_, i) => ({
        ...template,
        id: 5_000_000 + i,
        source_path: `${ROOT}/F${i % FOLDERS}/Track ${i}.mp3`,
        title: `Track ${i}`,
        has_analysis: false,
        analysis_pending: false,
      }));
      await page.evaluate((t) => {
        const w = window as unknown as SmokeWindow;
        w.__smokeImport = { tracks: t as never, already: 0 };
        w.__smokeHoldImport = true;
        w.__smokeAnalyzeAfterImport = true;
      }, tracks);

      await page.evaluate(() => {
        const w = window as unknown as { __marks: Record<string, number> };
        const seen = () => {
          if (!w.__marks.pending && document.querySelector('[data-pending-row]')) {
            w.__marks.pending = performance.now();
            requestAnimationFrame(() => requestAnimationFrame(() => (w.__marks.pendingFrame = performance.now())));
          }
          if (w.__marks.released && !w.__marks.real && !document.querySelector('[data-pending-row]') && document.querySelector('[data-track-row]')) {
            w.__marks.real = performance.now();
            requestAnimationFrame(() => requestAnimationFrame(() => (w.__marks.realFrame = performance.now())));
          }
        };
        new MutationObserver(seen).observe(document.body, { childList: true, subtree: true });
      });

      await page.locator('.ff-sidebar__fill').scrollIntoViewIfNeeded();
      const box = (await page.locator('.ff-sidebar__fill').boundingBox())!;
      const position = { x: Math.round(box.x + box.width / 2), y: Math.round(box.y + box.height / 2) };
      const profiler = process.env.FF_PERF_PROFILE_JSON ? await page.context().newCDPSession(page) : null;
      if (profiler) {
        await profiler.send('Profiler.enable');
        await profiler.send('Profiler.setSamplingInterval', { interval: 200 });
        await profiler.send('Profiler.start');
      }
      const tracePath = process.env.FF_PERF_TRACE;
      if (tracePath) await browser.startTracing(page, { categories: ['devtools.timeline', 'disabled-by-default-devtools.timeline', 'v8', 'blink.user_timing'] });
      const scriptBefore = await script();
      const taskBefore = await script('TaskDuration');
      await page.evaluate(
        ({ paths, position }) => {
          const w = window as unknown as SmokeWindow;
          w.__smokeEmit('tauri://drag-enter', { paths, position });
          w.__smokeEmit('tauri://drag-over', { position });
        },
        { paths: [ROOT], position },
      );
      await page.waitForTimeout(100);
      const t0 = await page.evaluate(
        ({ paths, position }) => {
          const w = window as unknown as SmokeWindow & { __marks: Record<string, number> };
          w.__marks.drop = performance.now();
          w.__smokeEmit('tauri://drag-drop', { paths, position });
          return w.__marks.drop;
        },
        { paths: [ROOT], position },
      );
      await page.waitForFunction(() => (window as unknown as { __marks: Record<string, number> }).__marks.pendingFrame !== undefined, null, { timeout: 60_000 });
      const scriptPending = (await script()) - scriptBefore;
      await page.waitForTimeout(800);
      await page.evaluate(() => {
        const w = window as unknown as SmokeWindow & { __marks: Record<string, number> };
        w.__marks.released = performance.now();
        w.__smokeReleaseImport?.();
      });
      await page.waitForFunction(() => (window as unknown as { __marks: Record<string, number> }).__marks.realFrame !== undefined, null, { timeout: 60_000 });
      const scriptReal = (await script()) - scriptBefore - scriptPending;
      if (tracePath) writeFileSync(tracePath, await browser.stopTracing());
      if (profiler) {
        const { profile } = (await profiler.send('Profiler.stop')) as { profile: unknown };
        writeFileSync(process.env.FF_PERF_PROFILE_JSON!, JSON.stringify(profile));
      }
      const idleBefore = await script();
      await page.waitForTimeout(IDLE_MS);
      const afterIdleScript = (await script()) - idleBefore;
      const rowsHash = sha(await visibleRows(page));
      const playlists = await savedPlaylists(page);
      const playlistsHash = sha(playlists);
      await page.evaluate(() => {
        const w = window as unknown as { __inputAt: number };
        for (const type of ['keydown', 'pointerdown']) document.addEventListener(type, () => (w.__inputAt = performance.now()), true);
      });
      const settle = () =>
        page.evaluate(
          () =>
            new Promise<number>((resolve) => {
              const t0 = (window as unknown as { __inputAt: number }).__inputAt;
              setTimeout(() => requestAnimationFrame(() => requestAnimationFrame(() => resolve(performance.now() - t0))), 0);
            }),
        );
      const steps: Record<string, number> = {};
      const step = async (name: string, action: () => Promise<void>) => {
        const before = await script();
        await action();
        steps[`${name} ms`] = Math.round(await settle());
        steps[`${name} script`] = Math.round((await script()) - before);
        await page.waitForTimeout(700);
      };
      await step('first sort title', () => page.locator('.ff-colh__cell[data-col-key="title"] .ff-colh__label').click());
      await step('first sort artist', () => page.locator('.ff-colh__cell[data-col-key="artist"] .ff-colh__label').click());
      await page.locator('.ff-gheader__search input').click();
      await step('first key', () => page.keyboard.press('a'));
      await step('second key', () => page.keyboard.press('m'));
      const marks = await page.evaluate(() => {
        const w = window as unknown as { __marks: Record<string, number>; __longTasks: { start: number; duration: number }[] };
        return { marks: w.__marks, longTasks: w.__longTasks };
      });
      const m = marks.marks;
      const longBetween = (from: number, to: number) => marks.longTasks.filter((l) => l.start >= from && l.start < to);
      const pendingTasks = longBetween(t0, m.released);
      const realTasks = longBetween(m.released, m.realFrame + 50);
      const out = {
        label: LABEL,
        run,
        idleMs: IDLE_MS,
        files: FILES,
        library: initial.tracks.length,
        dropToPendingRows: Math.round(m.pending - t0),
        dropToPendingFrame: Math.round(m.pendingFrame - t0),
        pendingScript: Math.round(scriptPending),
        pendingLongest: Math.round(Math.max(0, ...pendingTasks.map((l) => l.duration))),
        releaseToRealRows: Math.round(m.real - m.released),
        releaseToRealFrame: Math.round(m.realFrame - m.released),
        realScript: Math.round(scriptReal),
        realLongest: Math.round(Math.max(0, ...realTasks.map((l) => l.duration))),
        realLongTotal: Math.round(realTasks.reduce((a, l) => a + l.duration, 0)),
        taskMs: Math.round((await script('TaskDuration')) - taskBefore),
        afterIdleScript: Math.round(afterIdleScript),
        rowsHash,
        playlistsHash,
        ...steps,
      };
      if (REPORT) appendFileSync(REPORT, JSON.stringify(out) + '\n');
      else console.log(JSON.stringify(out));
    });
  }
});
