import { createWriteStream, mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import type { CDPSession, Page } from '@playwright/test';
import { expect, loadByDoubleClick, rows, sidebarPlaylist, test } from './app.ts';
import type { SmokeWindow } from './tauri-mock.ts';

const MINUTES = Number(process.env.FF_SOAK_MINUTES ?? 0);
const SAMPLE_SECONDS = Number(process.env.FF_SOAK_SAMPLE_SECS ?? 180);
const TRACKS = Number(process.env.FF_PERF_TRACKS ?? 0);
const REPORT = process.env.FF_SOAK_OUT;
const SNAPSHOT_DIR = process.env.FF_SOAK_SNAPSHOT_DIR;
const ANALYSED_IDS = 60;
const COVER_MIN_BYTES = 8192;
const COVER_URL_LIMIT = 200;
const WARMUP_CYCLES = Number(process.env.FF_SOAK_WARMUP ?? 3);

interface Sample {
  minute: number;
  cycle: number;
  heapMb: number;
  domNodes: number;
  listeners: number;
  documents: number;
  pageNodes: number;
  callbacks: number;
  invokes: number;
  timers: number;
  blobUrls: number;
  coverUrls: number;
}

async function sample(page: Page, cdp: CDPSession, started: number, cycle: number): Promise<Sample> {
  await cdp.send('HeapProfiler.collectGarbage');
  const { usedSize } = (await cdp.send('Runtime.getHeapUsage')) as { usedSize: number };
  const counters = (await cdp.send('Memory.getDOMCounters')) as { documents: number; nodes: number; jsEventListeners: number };
  const inPage = await page.evaluate((coverMinBytes) => {
    const w = window as unknown as SmokeWindow & Record<string, unknown>;
    const invokes = w.__smokeInvokes.length;
    w.__smokeInvokes.length = 0;
    return {
      pageNodes: document.getElementsByTagName('*').length,
      callbacks: Object.keys(w).filter((key) => /^_\d+$/.test(key)).length,
      invokes,
      timers: (window as unknown as { __soakTimers: Set<number> }).__soakTimers.size,
      blobUrls: (window as unknown as { __soakBlobUrls: Set<string> }).__soakBlobUrls.size,
      coverUrls: [...(window as unknown as { __soakBlobSizes: Map<string, number> }).__soakBlobSizes.values()].filter((bytes) => bytes >= coverMinBytes).length,
    };
  }, COVER_MIN_BYTES);
  return {
    minute: Math.round(((Date.now() - started) / 60_000) * 10) / 10,
    cycle,
    heapMb: Math.round(usedSize / 1e4) / 100,
    domNodes: counters.nodes,
    listeners: counters.jsEventListeners,
    documents: counters.documents,
    ...inPage,
  };
}

async function snapshot(cdp: CDPSession, path: string) {
  const out = createWriteStream(path);
  const onChunk = ({ chunk }: { chunk: string }) => out.write(chunk);
  cdp.on('HeapProfiler.addHeapSnapshotChunk', onChunk);
  await cdp.send('HeapProfiler.collectGarbage');
  await cdp.send('HeapProfiler.takeHeapSnapshot', { reportProgress: false });
  cdp.off('HeapProfiler.addHeapSnapshotChunk', onChunk);
  await new Promise<void>((done) => out.end(done));
}

async function fling(page: Page) {
  await page.evaluate(
    () =>
      new Promise<void>((resolve) => {
        const el = document.querySelector('.ff-table__scroll') as HTMLElement;
        let n = 0;
        const tick = () => {
          el.scrollTop = n % 2 === 0 ? el.scrollTop + 9000 : el.scrollTop - 4000;
          if (++n < 24) requestAnimationFrame(tick);
          else resolve();
        };
        requestAnimationFrame(tick);
      }),
  );
}

async function selectManyTracks(page: Page) {
  const scroll = page.locator('.ff-table__scroll');
  await scroll.evaluate((el) => (el.scrollTop = 0));
  for (let screen = 0; screen < 8; screen++) {
    const boxes = await page.locator('[data-track-row]').evaluateAll((els) =>
      els.map((el) => el.getBoundingClientRect()).filter((r) => r.height > 0).map((r) => ({ x: r.x + 120, y: r.y + r.height / 2 })),
    );
    for (const box of boxes) {
      await page.mouse.click(box.x, box.y);
      await page.waitForTimeout(20);
    }
    await scroll.evaluate((el) => (el.scrollTop += el.clientHeight - 60));
  }
}

async function searchTypeAndClear(page: Page, round: number) {
  const search = page.locator('.ff-gheader__search input');
  await search.click();
  const query = ['smoke track 1', 'solomun', 'melodic', 'bicep 12'][round % 4];
  for (const ch of query) await page.keyboard.press(ch === ' ' ? 'Space' : ch);
  await search.fill('');
  await page.keyboard.press('Escape');
  await expect(page.locator('.ff-cp')).toHaveCount(0);
}

async function paletteOpenClose(page: Page) {
  await page.keyboard.press('Control+k');
  await expect(page.locator('.ff-cp')).toBeVisible();
  await page.keyboard.type('set');
  await page.keyboard.press('Escape');
  await page.locator('.ff-gheader__search input').fill('');
  await page.keyboard.press('Escape');
  await expect(page.locator('.ff-cp')).toHaveCount(0);
}

async function switchPlaylists(page: Page) {
  const names = TRACKS ? ['Big Playlist', 'Peak Time', 'Warmup'] : ['Peak Time', 'Warmup'];
  for (const name of names) {
    await sidebarPlaylist(page, name).click();
    await expect(page.locator('.ff-gheader__count')).toBeVisible();
  }
  await page.locator('.ff-sbrow__label', { hasText: /^All Tracks$/ }).click();
}

async function loadAndSeek(page: Page, round: number) {
  await page.evaluate(() => ((document.querySelector('.ff-table__scroll') as HTMLElement).scrollTop = 0));
  const title = `Smoke Track ${String((round % 12) + 1).padStart(2, '0')}`;
  await loadByDoubleClick(page, title);
  await page.locator('.ff-player-compact__expand-btn').click();
  await expect(page.locator('.ff-xp .waveform-display__zoom')).toHaveAttribute('data-bands', 'detail');
  const overview = (await page.locator('.ff-xp .waveform-display__overview').boundingBox())!;
  for (const fraction of [0.2, 0.55, 0.8, 0.35]) {
    await page.mouse.click(overview.x + overview.width * fraction, overview.y + overview.height / 2);
    await page.waitForTimeout(120);
  }
  await page.keyboard.press('Escape');
  await expect(page.locator('.ff-xp')).toHaveCount(0);
}

async function settingsOpenClose(page: Page) {
  await page.keyboard.press('Control+,');
  const items = page.locator('.ff-settings-sheet__nav-item');
  await expect(items.first()).toBeVisible();
  const count = await items.count();
  for (let i = 0; i < count; i++) await items.nth(i).click();
  await page.keyboard.press('Escape');
  await expect(items.first()).toHaveCount(0);
}

async function analysisEvents(page: Page, round: number) {
  const ids = Array.from({ length: 40 }, (_, i) => ((round * 7 + (i + 1) * 13) % ANALYSED_IDS) + 1);
  await page.evaluate(
    (ids) =>
      new Promise<void>((resolve) => {
        const send = (window as unknown as SmokeWindow).__smokeEmit;
        let i = 0;
        const next = () => {
          const id = ids[i++];
          send('analysis-partial', { track_id: id });
          send('analysis-progress', { current: i, total: ids.length, message: `Analyzed Smoke Track ${id}.mp3 (${i}/${ids.length})`, track_id: id });
          if (i < ids.length) setTimeout(next, 25);
          else {
            send('analysis-complete', null);
            setTimeout(resolve, 150);
          }
        };
        next();
      }),
    ids,
  );
}

test.describe('soak', () => {
  test.use({ actionTimeout: 30_000 });
  test.skip(MINUTES === 0, 'set FF_SOAK_MINUTES=45 (and FF_PERF_TRACKS=10000) to run');
  test.setTimeout((MINUTES + 15) * 60_000);

  test('long session leaves no growth after GC', async ({ smoke: { page } }) => {
    test.setTimeout((MINUTES + 15) * 60_000);
    await page.evaluate(() => {
      const w = window as unknown as SmokeWindow & { __soakTimers: Set<number> };
      w.__smokeDetail = 'serve';
      w.__smokeArtwork = true;
      w.__soakTimers = new Set();
      const blobUrls = new Set<string>();
      const blobSizes = new Map<string, number>();
      (w as unknown as { __soakBlobUrls: Set<string> }).__soakBlobUrls = blobUrls;
      (w as unknown as { __soakBlobSizes: Map<string, number> }).__soakBlobSizes = blobSizes;
      const create = URL.createObjectURL.bind(URL);
      const revoke = URL.revokeObjectURL.bind(URL);
      URL.createObjectURL = (source: Blob | MediaSource) => {
        const url = create(source);
        blobUrls.add(url);
        blobSizes.set(url, (source as Blob).size ?? 0);
        return url;
      };
      URL.revokeObjectURL = (url: string) => {
        blobUrls.delete(url);
        blobSizes.delete(url);
        revoke(url);
      };
      const set = window.setInterval.bind(window);
      const clear = window.clearInterval.bind(window);
      window.setInterval = ((handler: TimerHandler, delay?: number, ...args: unknown[]) => {
        const id = set(handler, delay, ...args);
        w.__soakTimers.add(id);
        return id;
      }) as typeof window.setInterval;
      window.clearInterval = ((id?: number) => {
        if (id !== undefined) w.__soakTimers.delete(id);
        clear(id);
      }) as typeof window.clearInterval;
    });
    const cdp = await page.context().newCDPSession(page);
    await cdp.send('Memory.getDOMCounters');
    if (SNAPSHOT_DIR) mkdirSync(SNAPSHOT_DIR, { recursive: true });

    const all: [string, (round: number) => Promise<void>][] = [
      ['fling', () => fling(page)],
      ['search', (round) => searchTypeAndClear(page, round)],
      ['palette', () => paletteOpenClose(page)],
      ['playlists', () => switchPlaylists(page)],
      ['load and seek', (round) => loadAndSeek(page, round)],
      ['select tracks', () => selectManyTracks(page)],
      ['settings', () => settingsOpenClose(page)],
      ['analysis events', (round) => analysisEvents(page, round)],
    ];

    const only = process.env.FF_SOAK_ACTIONS?.split(',');
    const actions = only ? all.filter(([name]) => only.includes(name)) : all;
    expect(actions.length, `FF_SOAK_ACTIONS must name some of: ${all.map(([name]) => name).join(', ')}`).toBeGreaterThan(0);
    const samples: Sample[] = [];
    const warmup = actions.length * 2;
    let cycle = 0;
    let started = Date.now();
    let nextSample = 0;
    let tookMiddle = false;
    const deadline = () => Date.now() - started >= MINUTES * 60_000;

    while (true) {
      for (const [name, run] of actions) {
        const at = Date.now();
        await run(cycle).catch((error) => {
          throw new Error(`soak action "${name}" failed in cycle ${cycle}: ${error}`);
        });
        if (process.env.FF_SOAK_VERBOSE) console.log(`soak step ${cycle} ${name} ${Date.now() - at} ms`);
      }
      cycle++;
      if (cycle === WARMUP_CYCLES) {
        started = Date.now();
        samples.push(await sample(page, cdp, started, cycle));
        if (SNAPSHOT_DIR) await snapshot(cdp, join(SNAPSHOT_DIR, 'first.heapsnapshot'));
        nextSample = SAMPLE_SECONDS * 1000;
        console.log(`soak baseline ${JSON.stringify(samples.at(-1))} (${warmup} warmup actions)`);
        continue;
      }
      if (cycle > WARMUP_CYCLES && Date.now() - started >= nextSample) {
        samples.push(await sample(page, cdp, started, cycle));
        nextSample += SAMPLE_SECONDS * 1000;
        console.log(`soak sample ${JSON.stringify(samples.at(-1))}`);
        if (SNAPSHOT_DIR && !tookMiddle && samples.at(-1)!.minute >= MINUTES / 2) {
          tookMiddle = true;
          await snapshot(cdp, join(SNAPSHOT_DIR, 'middle.heapsnapshot'));
        }
      }
      if (cycle > WARMUP_CYCLES && deadline()) break;
    }
    const last = await sample(page, cdp, started, cycle);
    samples.push(last);
    if (SNAPSHOT_DIR) await snapshot(cdp, join(SNAPSHOT_DIR, 'last.heapsnapshot'));
    console.log(`soak end ${JSON.stringify(last)}`);

    const first = samples[0];
    const growth = {
      cycles: cycle,
      heapMb: Math.round((last.heapMb - first.heapMb) * 100) / 100,
      heapMbPerCycle: Math.round(((last.heapMb - first.heapMb) / Math.max(1, cycle - WARMUP_CYCLES)) * 1000) / 1000,
      heapMbSecondHalf: Math.round((last.heapMb - (samples.find((x) => x.minute >= MINUTES / 2) ?? first).heapMb) * 100) / 100,
      domNodes: last.domNodes - first.domNodes,
      listeners: last.listeners - first.listeners,
      documents: last.documents - first.documents,
      callbacks: last.callbacks - first.callbacks,
      timers: last.timers - first.timers,
      blobUrls: last.blobUrls - first.blobUrls,
      coverUrls: last.coverUrls,
    };
    console.log(`soak growth ${JSON.stringify(growth)}`);
    if (REPORT) writeFileSync(REPORT, JSON.stringify({ samples, growth }, null, 2));

    expect(growth.listeners, 'event listeners after the same actions').toBeLessThanOrEqual(10);
    expect(growth.documents, 'documents').toBeLessThanOrEqual(0);
    expect(growth.timers, 'intervals left running').toBeLessThanOrEqual(0);
    expect(last.blobUrls, 'object URLs alive').toBeLessThanOrEqual(2600);
    expect(Math.max(...samples.map((x) => x.coverUrls)), 'cover object URLs alive').toBeLessThanOrEqual(COVER_URL_LIMIT);
    expect(rows(page).first()).toBeVisible();
  });
});
