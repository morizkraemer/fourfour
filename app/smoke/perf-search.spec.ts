import { createHash } from 'node:crypto';
import { appendFileSync, readFileSync, writeFileSync } from 'node:fs';
import { expect, test, type Page } from '@playwright/test';
import type { LoadedState } from '../src/services/tauri.svelte.ts';
import { decodeTrackTable, isTrackTable } from '../src/services/track-table.ts';
import { mockAnalysisData } from '../src/mocks/analysis.ts';
import type { SmokeWindow } from './tauri-mock.ts';
import { installTauriMock } from './tauri-mock.ts';

const STATE = process.env.FF_PERF_STATE;
const REPORT = process.env.FF_PERF_OUT;
const RUNS = Number(process.env.FF_PERF_RUNS ?? 1);
const FAST_RATE = Number(process.env.FF_PERF_FAST_RATE ?? 6);
const LABEL = process.env.FF_PERF_LABEL ?? '';

interface Step {
  name: string;
  total: number;
  script: number;
  task: number;
  layout: number;
  view: string;
}

function state(): LoadedState {
  const dumped = JSON.parse(readFileSync(STATE!, 'utf8'));
  const loaded = { ...dumped, tracks: isTrackTable(dumped.tracks) ? decodeTrackTable(dumped.tracks) : dumped.tracks } as LoadedState;
  if (!process.env.FF_PERF_FRESH) return loaded;
  return { ...loaded, tracks: loaded.tracks.map((t) => ({ ...t, tempo: 0, key: '', has_analysis: false, has_cues: false })) };
}

async function metrics(page: Page) {
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

type Profile = { nodes: { id: number; children?: number[]; callFrame: { functionName: string; url: string; lineNumber: number } }[]; samples: number[]; timeDeltas: number[] };

async function startProfile(page: Page) {
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Profiler.enable');
  await cdp.send('Profiler.setSamplingInterval', { interval: 100 });
  await cdp.send('Profiler.start');
  return async () => {
    const { profile } = (await cdp.send('Profiler.stop')) as { profile: Profile };
    if (process.env.FF_PERF_PROFILE_JSON) writeFileSync(process.env.FF_PERF_PROFILE_JSON, JSON.stringify(profile));
    const self = new Map<string, number>();
    const parent = new Map<number, number>();
    for (const n of profile.nodes) for (const c of n.children ?? []) parent.set(c, n.id);
    const byId = new Map(profile.nodes.map((n) => [n.id, n]));
    const label = (id: number) => {
      const f = byId.get(id)!.callFrame;
      return `${f.functionName || '(anon)'} ${f.url.split('/').pop()}:${f.lineNumber}`;
    };
    const inclusive = new Map<string, number>();
    profile.samples.forEach((id, i) => {
      const ms = profile.timeDeltas[i] / 1000;
      self.set(label(id), (self.get(label(id)) ?? 0) + ms);
      const seen = new Set<string>();
      for (let at: number | undefined = id; at !== undefined; at = parent.get(at)) {
        const key = label(at);
        if (seen.has(key)) continue;
        seen.add(key);
        inclusive.set(key, (inclusive.get(key) ?? 0) + ms);
      }
    });
    const top = (m: Map<string, number>) => [...m.entries()].sort((a, b) => b[1] - a[1]).slice(0, Number(process.env.FF_PERF_TOP ?? 45)).map(([k, v]) => `${v.toFixed(0).padStart(7)} ms  ${k}`).join('\n');
    writeFileSync(process.env.FF_PERF_PROFILE_OUT ?? '/dev/stdout', `profile self\n${top(self)}\nprofile inclusive\n${top(inclusive)}\n`);
    await cdp.detach();
  };
}

async function runScenario(page: Page, mode: string, read: () => Promise<{ script: number; task: number; layout: number }>, steps: Step[]) {
  const search = page.locator('.ff-gheader__search input');
  const measure = async (name: string, action: () => Promise<void>, gap = 120) => {
    const before = await read();
    await action();
    const total = await settle(page);
    const after = await read();
    const view = sha(await topRows(page));
    steps.push({ name: `${mode} ${name}`, total, script: after.script - before.script, task: after.task - before.task, layout: after.layout - before.layout, view });
    await page.waitForTimeout(gap);
  };
  const type = async (query: string, label: string) => {
    for (let i = 0; i < query.length; i++) {
      const ch = query[i];
      await measure(`${label} key ${i + 1} '${ch}'`, () => page.keyboard.press(ch === ' ' ? 'Space' : ch));
    }
  };
  const checkpoint = async (name: string) => {
    const view = await fullView(page);
    steps.push({ name: `${mode} VIEW ${name}`, total: 0, script: 0, task: 0, layout: 0, view });
  };
  const sortBy = (key: string, name: string) => measure(`sort ${name}`, () => page.locator(`.ff-colh__cell[data-col-key="${key}"] .ff-colh__label`).click(), 400);

  await search.click();
  await type('ghosts of lagos', 'type');
  await checkpoint('after ghosts of lagos');
  for (let i = 0; i < 6; i++) await measure(`backspace ${i + 1}`, () => page.keyboard.press('Backspace'));
  await checkpoint('after backspaces');
  await measure('clear', () => search.fill(''), 400);
  await type('house', 'type house');
  await checkpoint('after house');
  await measure('clear', () => search.fill(''), 400);
  await page.keyboard.press('Escape');
  await page.locator('.ff-table__scroll').click({ position: { x: 5, y: 5 } }).catch(() => undefined);

  await sortBy('artist', 'artist asc');
  await checkpoint('sorted artist asc');
  await search.click();
  await type('amor bells', 'sorted type');
  await checkpoint('sorted amor bells');
  for (let i = 0; i < 4; i++) await measure(`sorted backspace ${i + 1}`, () => page.keyboard.press('Backspace'));
  await checkpoint('sorted after backspaces');
  await measure('sorted clear', () => search.fill(''), 400);
  await page.keyboard.press('Escape');
  await sortBy('artist', 'artist desc');
  await checkpoint('sorted artist desc');
  await sortBy('title', 'title asc');
  await checkpoint('sorted title asc');
  await sortBy('bpm', 'bpm asc');
  await checkpoint('sorted bpm asc');
  await sortBy('key', 'key asc');
  await checkpoint('sorted key asc');
  await search.click();
  await measure('filtered then sort: type a', () => page.keyboard.press('a'));
  await page.keyboard.press('Escape');
  await expect(page.locator('.ff-cp')).toHaveCount(0);
  await sortBy('title', 'title asc, filtered a');
  await sortBy('title', 'title desc, filtered a');
  await checkpoint('sorted filtered a');
  await measure('clear', () => search.fill(''), 400);
  await page.keyboard.press('Escape');
}

test.describe('search and sort latency', () => {
  test.skip(!STATE, 'set FF_PERF_STATE=/path/state.json (bench_startup dump)');
  test.setTimeout(900_000);

  for (let run = 0; run < RUNS; run++) {
    test(`run ${run}`, async ({ page }) => {
      await page.exposeFunction('__smokeAnalysis', (trackId: number) => mockAnalysisData(trackId, 180 + ((trackId * 7) % 300), 12400, '8A'));
      await page.addInitScript(installTauriMock, state());
      await page.goto('/');
      await page.locator('[data-track-row]').first().waitFor({ timeout: 120_000 });
      await page.waitForTimeout(3000);
      await page.evaluate(() => {
        const w = window as unknown as { __inputAt: number };
        for (const type of ['keydown', 'pointerdown']) document.addEventListener(type, () => (w.__inputAt = performance.now()), true);
        const smoke = window as unknown as SmokeWindow;
        smoke.__smokeAnalysis = (trackId) => (smoke.__smokeAnalysis as never) && Promise.resolve({ track_id: trackId });
      });
      await page.exposeFunction('__ffNoop', () => undefined);
      const read = await metrics(page);
      const steps: Step[] = [];

      const profiling = process.env.FF_PERF_PROFILE === 'idle' ? await startProfile(page) : null;
      await runScenario(page, 'idle', read, steps);
      if (profiling) await profiling();

      await page.keyboard.press('Control+k');
      await page.keyboard.type('Analyze all unanalyzed');
      await page.keyboard.press('Enter');
      await expect(page.locator('.ff-cp')).toHaveCount(0);
      await page.locator('.ff-gheader__search input').fill('');
      await page.keyboard.press('Escape');
      await page.waitForTimeout(1000);
      const total = await page.evaluate(() => document.querySelectorAll('[data-track-row]').length);
      expect(total).toBeGreaterThan(0);
      await page.evaluate(
        ({ rate, count }) => {
          const w = window as unknown as SmokeWindow & { __emitter?: number; __cursor?: number };
          w.__cursor = 0;
          w.__emitter = window.setInterval(() => {
            const n = w.__cursor!++;
            w.__smokeEmit('analysis-partial', { track_id: ((Math.floor(n / 2) * 7919) % count) + 1 });
          }, 1000 / rate);
        },
        { rate: FAST_RATE, count: 10_000 },
      );
      const profilingAnalysis = process.env.FF_PERF_PROFILE === 'analysis' ? await startProfile(page) : null;
      await runScenario(page, 'analysis', read, steps);
      if (profilingAnalysis) await profilingAnalysis();
      await page.evaluate(() => clearInterval((window as unknown as { __emitter: number }).__emitter));

      const line = JSON.stringify({ label: LABEL, run, steps });
      if (REPORT) appendFileSync(REPORT, line + '\n');
      else console.log(line);
    });
  }
});
