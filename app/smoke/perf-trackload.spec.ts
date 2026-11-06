import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { expect, test, type Page } from '@playwright/test';
import type { LoadedState } from '../src/services/tauri.svelte.ts';
import { decodeTrackTable, isTrackTable } from '../src/services/track-table.ts';
import { installTauriMock } from './tauri-mock.ts';

const STATE = process.env.FF_R128_STATE;
const DUMPS = process.env.FF_R128_DUMPS;
const ROWS = Number(process.env.FF_R128_ROWS ?? 0);
const RUNS = Number(process.env.FF_R128_RUNS ?? 7);
const MODE = process.env.FF_R128_MODE ?? 'timing';
const OUT = process.env.FF_R128_OUT;
const GAP_MS = Number(process.env.FF_R128_GAP_MS ?? 0);

const median = (values: number[]) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
const sha = (data: string | Buffer) => createHash('sha1').update(data).digest('hex').slice(0, 12);

function state(): LoadedState {
  const dumped = JSON.parse(readFileSync(STATE!, 'utf8'));
  const tracks = isTrackTable(dumped.tracks) ? decodeTrackTable(dumped.tracks) : dumped.tracks;
  return { ...dumped, tracks: ROWS > 0 ? tracks.slice(0, ROWS) : tracks } as LoadedState;
}

function dumpsOf(dir: string) {
  const count = readdirSync(dir).filter((f) => f.endsWith('.analysis')).length;
  return Array.from({ length: count }, (_, k) => ({
    analysis: readFileSync(`${dir}/${k}.analysis`),
    detail: readFileSync(`${dir}/${k}.detail`),
  }));
}

interface Probes {
  analysisAnsweredAt: number;
  detailAnsweredAt: number;
  parseMs: number;
  longTasks: { start: number; duration: number }[];
}

function installR128() {
  const w = window as unknown as {
    __r128: { ready: boolean; analysis: (string | ArrayBuffer)[]; detail: ArrayBuffer[] } & Probes;
    __TAURI_INTERNALS__: { invoke: (cmd: string, args: Record<string, unknown>) => Promise<unknown> };
  };
  w.__r128 = { ready: false, analysis: [], detail: [], analysisAnsweredAt: 0, detailAnsweredAt: 0, parseMs: 0, longTasks: [] };
  new PerformanceObserver((list) => {
    for (const e of list.getEntries()) w.__r128.longTasks.push({ start: e.startTime, duration: e.duration });
  }).observe({ type: 'longtask', buffered: true });
  const inner = w.__TAURI_INTERNALS__.invoke;
  w.__TAURI_INTERNALS__.invoke = async (cmd, args = {}) => {
    const r = w.__r128;
    if (!r.ready || (cmd !== 'get_analysis_data' && cmd !== 'get_waveform_detail')) return inner(cmd, args);
    const k = ((args.trackId as number) - 1) % r.analysis.length;
    let answer: unknown;
    await new Promise((resolve) => setTimeout(resolve, 0));
    if (cmd === 'get_analysis_data') {
      const wire = r.analysis[k];
      const t = performance.now();
      answer = typeof wire === 'string' ? JSON.parse(wire) : wire.slice(0);
      r.parseMs = performance.now() - t;
      r.analysisAnsweredAt = performance.now();
    } else {
      answer = r.detail[k].slice(0);
      r.detailAnsweredAt = performance.now();
    }
    return answer;
  };
}

async function openApp(browser: import('@playwright/test').Browser, wire: ReturnType<typeof dumpsOf>) {
  const context = await browser.newContext({ viewport: { width: 1600, height: 1000 } });
  const page = await context.newPage();
  await page.exposeFunction('__smokeAnalysis', () => {
    throw new Error('r128: analysis comes from the dumps');
  });
  await page.addInitScript(installTauriMock, state());
  await page.addInitScript(installR128);
  await page.route('**/__r128/**', (route) => {
    const [, kind, k] = new URL(route.request().url()).pathname.match(/__r128\/(analysis|detail)\/(\d+)/)!;
    route.fulfill({ body: wire[Number(k)][kind as 'analysis' | 'detail'], contentType: 'application/octet-stream' });
  });
  await page.goto('/');
  await page.locator('[data-track-row]').first().waitFor({ timeout: 120_000 });
  await page.waitForTimeout(1500);
  await page.evaluate(async (count) => {
    const r = (window as unknown as { __r128: { ready: boolean; analysis: (string | ArrayBuffer)[]; detail: ArrayBuffer[] } }).__r128;
    for (let k = 0; k < count; k++) {
      const a = await fetch(`/__r128/analysis/${k}`);
      const bytes = await a.arrayBuffer();
      const first = new Uint8Array(bytes)[0];
      r.analysis[k] = first === 0x7b ? new TextDecoder().decode(bytes) : bytes;
      r.detail[k] = await (await fetch(`/__r128/detail/${k}`)).arrayBuffer();
    }
    r.ready = true;
    (window as unknown as SmokeWindowDetail).__smokeDetail = 'serve';
  }, wire.length);
  return { context, page };
}

interface SmokeWindowDetail {
  __smokeDetail: string;
}

function loadTrack(page: Page, rowIndex: number) {
  return page.evaluate(
    (rowIndex) =>
      new Promise<Record<string, number>>((resolve, reject) => {
        const r = (window as unknown as { __r128: Probes }).__r128;
        const row = document.querySelectorAll('[data-track-row]')[rowIndex] as HTMLElement;
        performance.clearMarks('ff:player-ready');
        const tasksBefore = r.longTasks.length;
        r.analysisAnsweredAt = 0;
        r.detailAnsweredAt = 0;
        const gaps: number[] = [];
        let last = performance.now();
        let watching = true;
        const frame = (now: number) => {
          gaps.push(now - last);
          last = now;
          if (watching) requestAnimationFrame(frame);
        };
        requestAnimationFrame(frame);
        const t0 = performance.now();
        row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true }));
        let ready = 0;
        let painted = 0;
        const poll = () => {
          const now = performance.now();
          if (now - t0 > 30_000) return reject(new Error(`track load timed out (ready ${ready}, painted ${painted})`));
          const mark = performance.getEntriesByName('ff:player-ready').at(-1);
          if (mark && !ready) {
            ready = mark.startTime - t0;
            requestAnimationFrame(() => requestAnimationFrame(() => (painted = performance.now() - t0)));
          }
          if (painted && r.detailAnsweredAt > t0) {
            return setTimeout(() => {
              watching = false;
              const tasks = r.longTasks.slice(tasksBefore);
              resolve({
                analysisAnswered: r.analysisAnsweredAt ? r.analysisAnsweredAt - t0 : 0,
                ready,
                painted,
                detailAnswered: r.detailAnsweredAt - t0,
                longTaskMs: tasks.reduce((s, t) => s + t.duration, 0),
                longestTask: tasks.reduce((m, t) => Math.max(m, t.duration), 0),
                worstFrame: Math.max(...gaps),
                parseMs: r.parseMs,
              });
            }, 400);
          }
          setTimeout(poll, 0);
        };
        poll();
      }),
    rowIndex,
  );
}

async function pressRowFirst(page: Page, rowIndex: number, gapMs: number) {
  await page.evaluate(
    ([rowIndex, gapMs]) =>
      new Promise<void>((resolve) => {
        const row = document.querySelectorAll('[data-track-row]')[rowIndex] as HTMLElement;
        const press = () => {
          row.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, cancelable: true, button: 0, pointerType: 'mouse', isPrimary: true }));
          row.dispatchEvent(new PointerEvent('pointerup', { bubbles: true, cancelable: true, button: 0, pointerType: 'mouse', isPrimary: true }));
          row.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, detail: 1 }));
        };
        press();
        setTimeout(resolve, gapMs);
      }),
    [rowIndex, gapMs] as const,
  );
}

type Profile = { nodes: { id: number; children?: number[]; callFrame: { functionName: string; url: string; lineNumber: number } }[]; samples: number[]; timeDeltas: number[] };

function summariseProfile(profiles: Profile[]) {
  const self = new Map<string, number>();
  let busy = 0;
  for (const profile of profiles) {
    const byId = new Map(profile.nodes.map((n) => [n.id, n]));
    profile.samples.forEach((id, i) => {
      const n = byId.get(id)!;
      const name = n.callFrame.functionName || '(anon)';
      if (name === '(idle)') return;
      const label = `${name} ${n.callFrame.url.split('/').pop()}:${n.callFrame.lineNumber}`;
      const ms = profile.timeDeltas[i] / 1000;
      self.set(label, (self.get(label) ?? 0) + ms);
      if (name !== '(program)') busy += ms;
    });
  }
  const top = [...self.entries()].sort((a, b) => b[1] - a[1]).slice(0, 14);
  console.log(`R128 profile busy ms per load ${(busy / profiles.length).toFixed(1)}\n${top.map(([l, ms]) => `${(ms / profiles.length).toFixed(1).padStart(8)} ms  ${l}`).join('\n')}`);
}

test.describe('r128 track load', () => {
  test.skip(!STATE || !DUMPS, 'set FF_R128_STATE=<state json> FF_R128_DUMPS=<dump-load dir>');
  test.setTimeout(900_000);

  test('timing', async ({ browser }) => {
    test.skip(MODE !== 'timing');
    const { context, page } = await openApp(browser, dumpsOf(DUMPS!));
    const runs: Record<string, number>[] = [];
    for (let i = 0; i < RUNS + 1; i++) {
      const index = 3 + (i % 24);
      if (GAP_MS > 0) await pressRowFirst(page, index, GAP_MS);
      const run = await loadTrack(page, index);
      if (i > 0) runs.push(run);
      await page.waitForTimeout(300);
    }
    const out: Record<string, number> = {};
    for (const key of Object.keys(runs[0])) out[key] = Math.round(median(runs.map((r) => r[key])) * 10) / 10;
    console.log(`R128 ${JSON.stringify({ rows: ROWS || 'all', gapMs: GAP_MS, dumps: DUMPS, ...out })}`);
    if (OUT) writeFileSync(OUT, JSON.stringify({ medians: out, runs }, null, 1));
    await context.close();
  });

  test('profile', async ({ browser }) => {
    test.skip(MODE !== 'profile');
    const { context, page } = await openApp(browser, dumpsOf(DUMPS!));
    const profiles: Profile[] = [];
    for (let i = 0; i < RUNS + 1; i++) {
      const index = 3 + (i % 24);
      const cdp = await page.context().newCDPSession(page);
      await cdp.send('Profiler.enable');
      await cdp.send('Profiler.setSamplingInterval', { interval: 100 });
      if (i > 0) await cdp.send('Profiler.start');
      await loadTrack(page, index);
      if (i > 0) profiles.push(((await cdp.send('Profiler.stop')) as { profile: Profile }).profile);
      await cdp.detach();
      await page.waitForTimeout(300);
    }
    summariseProfile(profiles);
    await context.close();
  });

  test('identity', async ({ browser }) => {
    test.skip(MODE !== 'identity');
    const { context, page } = await openApp(browser, dumpsOf(DUMPS!));
    const hashes: Record<string, string> = {};
    const canvasHashes = (selector: string) =>
      page.evaluate((selector) => Array.from(document.querySelectorAll(selector)).map((c) => (c as HTMLCanvasElement).toDataURL()), selector);
    for (const index of [3, 5, 7]) {
      await loadTrack(page, index);
      await page.waitForTimeout(300);
      if ((await page.locator('.ff-player-compact__play').getAttribute('aria-label').catch(() => null)) === 'Pause') await page.keyboard.press('Space');
      await page.waitForTimeout(300);
      hashes[`track${index} compact`] = (await canvasHashes('.ff-player-module canvas')).map(sha).join(' ');
      await page.locator('.ff-player-compact__expand-btn').click();
      await page.locator('.ff-xp .waveform-display__zoom[data-bands="detail"]').waitFor({ timeout: 30_000 });
      if ((await page.locator('.ff-xp__play').getAttribute('aria-label')) === 'Pause') await page.locator('.ff-xp__play').click();
      await page.waitForTimeout(300);
      const overview = page.locator('.ff-xp .waveform-display__overview');
      const box = (await overview.boundingBox())!;
      for (const frac of [0.02, 0.37, 0.731]) {
        await page.mouse.click(box.x + box.width * frac, box.y + box.height / 2);
        await page.waitForTimeout(300);
        const canvases = await canvasHashes('.ff-xp canvas');
        const shot = await page.locator('.ff-xp').screenshot();
        hashes[`track${index} seek${frac}`] = `${canvases.map(sha).join(' ')} ${sha(shot)}`;
      }
      await page.locator('.ff-xp__collapse').click();
      await expect(page.locator('.ff-xp')).toHaveCount(0);
    }
    writeFileSync(OUT ?? 'smoke/.results/r128-identity.json', JSON.stringify(hashes, null, 1));
    console.log(`R128 identity ${Object.keys(hashes).length} snapshots`);
    await context.close();
  });
});
