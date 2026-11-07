import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { appendFileSync, mkdirSync, readFileSync, renameSync } from 'node:fs';
import { loadavg } from 'node:os';
import { join } from 'node:path';
import { test, type Page } from '@playwright/test';
import type { LoadedState } from '../src/services/tauri.svelte.ts';
import { decodeTrackTable, isTrackTable } from '../src/services/track-table.ts';
import { mockAnalysisData } from '../src/mocks/analysis.ts';
import { installTauriMock } from './tauri-mock.ts';

const STATE = process.env.FF_PERF_EDGES;
const REPORT = process.env.FF_PERF_EDGES_OUT;
const VIDEO_DIR = process.env.FF_PERF_EDGES_VIDEO;
const LABEL = process.env.FF_PERF_LABEL ?? '';
const RUNS = Number(process.env.FF_PERF_RUNS ?? 1);
const PARTS = new Set((process.env.FF_PERF_EDGES_PARTS ?? 'wheel,raf,settle').split(','));
const WEBKIT = process.env.FF_SMOKE_BROWSER === 'webkit';
const ROW = 28;
const VIEW = { width: 1600, height: 1000 };

const round = (v: number) => Math.round(v * 10) / 10;
const sha = (data: string | Buffer) => createHash('sha1').update(data).digest('hex').slice(0, 12);
const percentile = (values: number[], p: number) => [...values].sort((a, b) => a - b)[Math.min(values.length - 1, Math.floor((values.length * p) / 100))] ?? 0;

function rowBands(i: number) {
  const columns = 96 + (i % 5) * 16;
  const bytes = new Uint8Array(columns * 3);
  for (let c = 0; c < columns; c++) {
    const swell = 0.35 + 0.65 * Math.abs(Math.sin((c + i * 3) / (9 + (i % 7))));
    bytes[c] = Math.round((40 + ((c * 7 + i) % 80)) * swell);
    bytes[columns + c] = Math.round((20 + ((c * 3 + i) % 60)) * swell);
    bytes[2 * columns + c] = Math.round((10 + ((c * 5 + i) % 40)) * swell);
  }
  return Buffer.from(bytes).toString('base64');
}

function state(): LoadedState {
  const dumped = JSON.parse(readFileSync(STATE!, 'utf8'));
  const tracks = isTrackTable(dumped.tracks) ? decodeTrackTable(dumped.tracks) : dumped.tracks;
  return { ...dumped, tracks: tracks.map((t: LoadedState['tracks'][number], i: number) => (i % 3 === 2 ? t : { ...t, waveform_bands: rowBands(i) })) } as LoadedState;
}

function report(kind: string, run: number, data: unknown) {
  const line = JSON.stringify({ label: LABEL, kind, run, browser: WEBKIT ? 'webkit' : 'chromium', load: loadavg().map(round), data });
  if (REPORT) appendFileSync(REPORT, `${line}\n`);
  else console.log(line);
}

interface FrameSample {
  t: number;
  top: number;
  pre: number;
  post: number;
}

function installEdgeProbe() {
  const w = window as unknown as { __edges: { on: boolean; frames: FrameSample[] } };
  w.__edges = { on: false, frames: [] };
  const blankSlots = () => {
    const scroller = document.querySelector('.ff-table__scroll') as HTMLElement | null;
    const body = document.querySelector('.ff-table__body') as HTMLElement | null;
    if (!scroller || !body) return { top: 0, blank: 0 };
    const header = (scroller.querySelector('.ff-colh') as HTMLElement | null)?.offsetHeight ?? 0;
    const top = scroller.scrollTop;
    const count = Number(body.dataset.rowCount ?? 0);
    const first = Math.floor(top / 28);
    const last = Math.min(count, Math.ceil((top + scroller.clientHeight - header) / 28)) - 1;
    const shown = new Set<number>();
    for (const row of body.querySelectorAll<HTMLElement>('[data-track-row]')) {
      if (row.style.display === 'none') continue;
      if (!row.querySelector('.ff-trow__cell--title .ff-trow__text')?.textContent) continue;
      shown.add(Number(row.dataset.rowIndex));
    }
    let blank = 0;
    for (let i = first; i <= last; i++) if (!shown.has(i)) blank++;
    return { top, blank };
  };
  let scrolled = false;
  let pre = 0;
  const post = () => {
    const sample = blankSlots();
    w.__edges.frames[w.__edges.frames.length - 1].post = sample.blank;
    void pre;
  };
  const frame = () => {
    requestAnimationFrame(frame);
    if (!w.__edges.on) return;
    const sample = blankSlots();
    pre = sample.blank;
    w.__edges.frames.push({ t: performance.now(), top: sample.top, pre: sample.blank, post: sample.blank });
    scrolled = false;
  };
  requestAnimationFrame(frame);
  const hook = () => {
    const scroller = document.querySelector('.ff-table__scroll');
    if (!scroller) return requestAnimationFrame(hook);
    scroller.addEventListener('scroll', () => {
      if (!w.__edges.on || scrolled) return;
      scrolled = true;
      requestAnimationFrame(() => w.__edges.frames.length && post());
    });
  };
  requestAnimationFrame(hook);
}

async function open(browser: import('@playwright/test').Browser, payload: LoadedState, video: boolean) {
  const context = await browser.newContext({ viewport: VIEW, deviceScaleFactor: Number(process.env.FF_PERF_EDGES_DPR ?? 1), ...(video && VIDEO_DIR ? { recordVideo: { dir: VIDEO_DIR, size: VIEW } } : {}) });
  const page = await context.newPage();
  const videoStart = Date.now();
  await page.exposeFunction('__smokeAnalysis', (trackId: number) => mockAnalysisData(trackId, 180 + ((trackId * 7) % 300), 12400, '8A'));
  await page.addInitScript(installTauriMock, payload);
  await page.addInitScript(installEdgeProbe);
  await page.goto('/');
  await page.locator('[data-track-row]').first().waitFor({ timeout: 120_000 });
  await page.waitForTimeout(4000);
  return { context, page, videoStart };
}

async function listBox(page: Page) {
  return page.evaluate(() => {
    const scroller = document.querySelector('.ff-table__scroll') as HTMLElement;
    const header = (scroller.querySelector('.ff-colh') as HTMLElement).getBoundingClientRect();
    const box = scroller.getBoundingClientRect();
    const title = scroller.querySelector('.ff-colh [data-col-key="title"]')?.getBoundingClientRect();
    const artist = scroller.querySelector('.ff-colh [data-col-key="artist"]')?.getBoundingClientRect();
    const left = title?.left ?? box.left;
    const right = artist?.right ?? title?.right ?? box.right;
    return { x: Math.round(left), y: Math.round(header.bottom + 1), w: Math.round(right - left), h: Math.round(box.bottom - header.bottom - 2) };
  });
}

async function wheelScroll(page: Page, steps: number, delta: number, gapMs: number) {
  const box = await page.locator('.ff-table__scroll').boundingBox();
  await page.mouse.move(box!.x + box!.width / 2, box!.y + box!.height / 2);
  const t0 = Date.now();
  for (let i = 0; i < steps; i++) {
    await page.mouse.wheel(0, delta);
    const due = t0 + (i + 1) * gapMs;
    const wait = due - Date.now();
    if (wait > 0) await page.waitForTimeout(wait);
  }
}

async function frames(page: Page, run: () => Promise<void>) {
  await page.evaluate(() => {
    const w = window as unknown as { __edges: { on: boolean; frames: FrameSample[] } };
    w.__edges.frames = [];
    w.__edges.on = true;
  });
  await run();
  await page.waitForTimeout(400);
  return page.evaluate(() => {
    const w = window as unknown as { __edges: { on: boolean; frames: FrameSample[] } };
    w.__edges.on = false;
    return w.__edges.frames;
  });
}

function summarise(samples: FrameSample[]) {
  const gaps = samples.slice(1).map((s, i) => s.t - samples[i].t);
  const moved = samples.slice(1).map((s, i) => Math.abs(s.top - samples[i].top));
  return {
    frames: samples.length,
    pxPerMs: round(moved.reduce((a, b) => a + b, 0) / Math.max(1, gaps.reduce((a, b) => a + b, 0))),
    preBlankFrames: samples.filter((s) => s.pre > 0).length,
    preBlankMax: Math.max(0, ...samples.map((s) => s.pre)),
    postBlankFrames: samples.filter((s) => s.post > 0).length,
    postBlankMax: Math.max(0, ...samples.map((s) => s.post)),
    postBlankSum: samples.reduce((a, s) => a + s.post, 0),
  };
}

function rafFling(page: Page, count: number) {
  return page.evaluate(
    (count) =>
      new Promise<number[]>((resolve) => {
        const el = document.querySelector('.ff-table__scroll') as HTMLElement;
        el.scrollTop = 0;
        const step = (el.scrollHeight - el.clientHeight) / count;
        const deltas: number[] = [];
        let last = performance.now();
        let n = 0;
        const tick = () => {
          const t = performance.now();
          deltas.push(t - last);
          last = t;
          el.scrollTop += step;
          if (++n < count) requestAnimationFrame(tick);
          else resolve(deltas);
        };
        requestAnimationFrame(tick);
      }),
    count,
  );
}

const visibleRowsText = (page: Page) =>
  page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>('[data-track-row]')]
      .filter((r) => r.style.display !== 'none')
      .sort((a, b) => Number(a.dataset.rowIndex) - Number(b.dataset.rowIndex))
      .map((r) => `${r.dataset.rowIndex}:${r.textContent}`)
      .join('\n'),
  );

const canvasHash = (page: Page) =>
  page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>('[data-track-row]')]
      .filter((r) => r.style.display !== 'none')
      .sort((a, b) => Number(a.dataset.rowIndex) - Number(b.dataset.rowIndex))
      .map((r) => {
        const c = r.querySelector('canvas');
        return c ? `${c.style.visibility}|${c.toDataURL().length}:${c.toDataURL().slice(-48)}` : '-';
      })
      .join('|'),
  );

function blankFromVideo(file: string, box: { x: number; y: number; w: number; h: number }) {
  const raw = execFileSync('ffmpeg', ['-v', 'error', '-i', file, '-vf', `crop=${box.w}:${box.h}:${box.x}:${box.y},format=gray`, '-f', 'rawvideo', '-'], { maxBuffer: 2 ** 31 });
  const size = box.w * box.h;
  const count = Math.floor(raw.length / size);
  const perFrame: number[] = [];
  for (let f = 0; f < count; f++) {
    const base = f * size;
    let gap = 0;
    let blank = 0;
    let seenInk = false;
    const lineInk = (y: number) => {
      const row = raw.subarray(base + y * box.w, base + (y + 1) * box.w);
      const sorted = Uint8Array.from(row).sort();
      const mid = sorted[sorted.length >> 1];
      let ink = 0;
      for (let x = 0; x < row.length; x++) if (Math.abs(row[x] - mid) > 40) ink++;
      return ink > 2;
    };
    for (let y = 0; y < box.h; y++) {
      if (lineInk(y)) {
        if (seenInk || gap > 0) blank += Math.max(0, Math.round((gap - 16) / ROW));
        gap = 0;
        seenInk = true;
      } else gap++;
    }
    blank += Math.max(0, Math.floor(gap / ROW));
    perFrame.push(blank);
  }
  return perFrame;
}

test.describe('scroll edges', () => {
  test.skip(!STATE, 'set FF_PERF_EDGES=/path/state.json');
  test.setTimeout(1_200_000);

  test('blank row slots and fling frames', async ({ browser }) => {
    const payload = state();
    for (let run = 0; run < RUNS; run++) {
      const { context, page, videoStart } = await open(browser, payload, run === 0);
      const box = await listBox(page);
      const scenarios: Record<string, { steps: number; delta: number; gap: number }> = {
        scroll: { steps: 60, delta: 100, gap: 16 },
        fast: { steps: 60, delta: 300, gap: 16 },
        fling: { steps: 40, delta: 1200, gap: 16 },
      };
      const videoMarks: { name: string; from: number; to: number }[] = [];
      const t0 = videoStart;
      const idleFrom = Date.now() - t0;
      if (PARTS.has('wheel')) {
        for (const [name, s] of Object.entries(scenarios)) {
          await page.evaluate(() => ((document.querySelector('.ff-table__scroll') as HTMLElement).scrollTop = 0));
          await page.waitForTimeout(800);
          const from = Date.now() - t0;
          const samples = await frames(page, () => wheelScroll(page, s.steps, s.delta, s.gap));
          videoMarks.push({ name, from, to: Date.now() - t0 });
          report(`wheel-${name}`, run, summarise(samples));
        }
      }
      if (PARTS.has('gesture') && !WEBKIT) {
        const cdp = await context.newCDPSession(page);
        const scroller = await page.locator('.ff-table__scroll').boundingBox();
        for (const speed of [2000, 6000, 15000]) {
          await page.evaluate(() => ((document.querySelector('.ff-table__scroll') as HTMLElement).scrollTop = 0));
          await page.waitForTimeout(800);
          const from = Date.now() - t0;
          const samples = await frames(page, async () => {
            await cdp.send('Input.synthesizeScrollGesture', {
              x: Math.round(scroller!.x + scroller!.width / 2),
              y: Math.round(scroller!.y + scroller!.height / 2),
              yDistance: -Math.round(speed * 1.5),
              speed,
              gestureSourceType: 'mouse',
              preventFling: true,
            });
          });
          videoMarks.push({ name: `gesture${speed}`, from, to: Date.now() - t0 });
          report(`gesture-${speed}`, run, summarise(samples));
        }
      }
      if (PARTS.has('raf')) {
        await page.waitForTimeout(800);
        const from = Date.now() - t0;
        let deltas: number[] = [];
        const samples = await frames(page, async () => {
          deltas = await rafFling(page, 90);
        });
        videoMarks.push({ name: 'raf', from, to: Date.now() - t0 });
        const f = deltas.slice(1);
        report('raf-fling', run, {
          ...summarise(samples),
          mean: round(f.reduce((a, b) => a + b, 0) / f.length),
          p50: round(percentile(f, 50)),
          p95: round(percentile(f, 95)),
          max: round(Math.max(...f)),
          over25: f.filter((d) => d > 25).length,
          over50: f.filter((d) => d > 50).length,
        });
      }
      if (PARTS.has('settle')) {
        const shots: string[] = [];
        await page.evaluate(() => ((document.querySelector('.ff-table__scroll') as HTMLElement).scrollTop = 0));
        await page.waitForTimeout(800);
        for (const target of [3, 12, 40]) {
          await wheelScroll(page, target, 100, 60);
          await page.mouse.move(5, 5);
          await page.waitForTimeout(1500);
          const top = await page.evaluate(() => (document.querySelector('.ff-table__scroll') as HTMLElement).scrollTop);
          const png = await page.screenshot({ animations: 'disabled' });
          shots.push(`top ${top} shot ${sha(png)} rows ${sha(await visibleRowsText(page))} waves ${sha(await canvasHash(page))}`);
        }
        await rafFling(page, 30);
        await page.waitForTimeout(1500);
        const top = await page.evaluate(() => (document.querySelector('.ff-table__scroll') as HTMLElement).scrollTop);
        shots.push(`after fling top ${top} shot ${sha(await page.screenshot({ animations: 'disabled' }))} rows ${sha(await visibleRowsText(page))} waves ${sha(await canvasHash(page))}`);
        report('settle', run, shots);
      }
      const video = page.video();
      await context.close();
      if (video && VIDEO_DIR) {
        const file = await video.path();
        const named = join(VIDEO_DIR, `${LABEL || 'run'}-${WEBKIT ? 'webkit' : 'chromium'}.webm`);
        mkdirSync(VIDEO_DIR, { recursive: true });
        renameSync(file, named);
        const perFrame = blankFromVideo(named, box);
        const fps = 25;
        const pixel: Record<string, unknown> = { box, frames: perFrame.length };
        for (const m of videoMarks) {
          const slice = perFrame.slice(Math.floor((m.from / 1000) * fps), Math.ceil((m.to / 1000) * fps));
          pixel[m.name] = { frames: slice.length, blankFrames: slice.filter((b) => b > 0).length, max: Math.max(0, ...slice), sum: slice.reduce((a, b) => a + b, 0) };
        }
        pixel.idle = perFrame.slice(Math.floor((idleFrom / 1000) * fps) - 25, Math.floor((idleFrom / 1000) * fps)).filter((b) => b > 0).length;
        report('video', run, { file: named, marks: videoMarks, pixel });
      }
    }
  });
});
