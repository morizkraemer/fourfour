import { execSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { expect, test, type Browser, type Page } from '@playwright/test';
import type { LoadedState } from '../src/services/tauri.svelte.ts';
import { decodeTrackTable, isTrackTable } from '../src/services/track-table.ts';
import { fixtureAnalysis, fixtureState } from './fixture.ts';
import { installTauriMock, type SmokeWindow } from './tauri-mock.ts';

const ENABLED = !!process.env.FF_PERF_R118;
const STATE = process.env.FF_PERF_STATE;
const OUT = process.env.FF_PERF_R118_OUT ?? 'smoke/.results/r118';
const TRACE_CATEGORIES = ['devtools.timeline', 'disabled-by-default-devtools.timeline', 'v8.execute', 'blink.user_timing'];

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
  const loaded = baseState();
  return { ...loaded, tracks: loaded.tracks.map((t, i) => (i % 3 === 2 ? t : { ...t, waveform_bands: rowBands(i) })) };
}

function baseState(): LoadedState {
  if (STATE) {
    const dumped = JSON.parse(readFileSync(STATE, 'utf8'));
    return { ...dumped, tracks: isTrackTable(dumped.tracks) ? decodeTrackTable(dumped.tracks) : dumped.tracks } as LoadedState;
  }
  return fixtureState();
}

async function openApp(browser: Browser, deviceScaleFactor = 1) {
  const context = await browser.newContext({ viewport: { width: 1600, height: 1000 }, deviceScaleFactor });
  const page = await context.newPage();
  await page.exposeFunction('__smokeAnalysis', (trackId: number) => fixtureAnalysis(trackId));
  await page.addInitScript(installTauriMock, state());
  await page.goto('/');
  await page.locator('[data-track-row]').first().waitFor({ timeout: 120_000 });
  await page.evaluate(() => ((window as unknown as SmokeWindow).__smokeDetail = 'serve'));
  await page.waitForTimeout(2500);
  return { context, page };
}

async function playAndExpand(page: Page, rowIndex: number) {
  await page.locator('[data-track-row]').nth(rowIndex).dblclick();
  await expect(page.locator('.ff-player-compact__meta-title')).not.toHaveText('');
  await page.waitForTimeout(1200);
  await page.locator('.ff-player-compact__expand-btn').click();
  await page.locator('.ff-xp .waveform-display__zoom[data-bands="detail"]').waitFor({ timeout: 30_000 });
  await page.waitForTimeout(800);
}

test('r118 layer tree', async ({ browser, browserName }) => {
  test.skip(!ENABLED || browserName !== 'chromium', 'set FF_PERF_R118=1 (chromium only)');
  const { context, page } = await openApp(browser);
  await playAndExpand(page, 3);
  if (process.env.FF_PERF_LAYER_CSS) await page.addStyleTag({ content: process.env.FF_PERF_LAYER_CSS });
  await page.waitForTimeout(300);
  const cdp = await context.newCDPSession(page);
  await cdp.send('DOM.enable');
  await cdp.send('LayerTree.enable');
  type Layer = { layerId: string; width: number; height: number; drawsContent: boolean; backendNodeId?: number };
  const layers = await new Promise<Layer[]>((resolve) => {
    cdp.on('LayerTree.layerTreeDidChange', (e: { layers?: Layer[] }) => e.layers && resolve(e.layers));
    void page.evaluate(() => document.body.classList.toggle('r118-poke'));
  });
  const drawing = layers.filter((l) => l.drawsContent);
  const reasons = new Map<string, number>();
  const byKind = new Map<string, number>();
  for (const l of drawing) {
    let compositingReasonIds: string[] = ['unknown'];
    try {
      compositingReasonIds = ((await cdp.send('LayerTree.compositingReasons', { layerId: l.layerId })) as { compositingReasonIds: string[] }).compositingReasonIds;
    } catch {}
    const key = compositingReasonIds.join('+') || 'none';
    reasons.set(key, (reasons.get(key) ?? 0) + 1);
    let kind = 'unknown';
    if (l.backendNodeId) {
      try {
        const { node } = (await cdp.send('DOM.describeNode', { backendNodeId: l.backendNodeId })) as { node: { nodeName: string; attributes?: string[] } };
        const cls = node.attributes?.[node.attributes.indexOf('class') + 1] ?? '';
        kind = `${node.nodeName.toLowerCase()}.${cls.split(' ')[0]}`;
      } catch {
        kind = 'gone';
      }
    }
    byKind.set(kind, (byKind.get(kind) ?? 0) + 1);
  }
  console.log(`layers ${layers.length}, drawing ${drawing.length}`);
  console.log('by reason', JSON.stringify([...reasons]));
  console.log('by node', JSON.stringify([...byKind].sort((a, b) => b[1] - a[1])));
  await context.close();
});

const VARIANT_CSS: Record<string, string> = {
  base: '',
  'no-row-canvas': '[data-track-row] canvas { display: none !important }',
  'row-no-contain': '.ff-table__row { contain: none !important }',
  'no-cursor': '.ff-trow__wave-cursor { display: none !important }',
  'no-cursor-no-canvas': '.ff-trow__wave-cursor, [data-track-row] canvas { display: none !important }',
  'hide-text': '.ff-trow__text { display: none !important }',
  'hide-cover': '.ff-trow__cell--cover { display: none !important }',
  'hide-wave': '.ff-trow__cell--wave { display: none !important }',
  'hide-rating': '.ff-trow__cell--rating { display: none !important }',
  'hide-all-cells': '.ff-trow__cell { display: none !important }',
  'no-clip': '.ff-trow__cell, .ff-trow__text { overflow: visible !important; text-overflow: clip !important }',
  'scroll-willchange': '.ff-table__scroll { will-change: transform }',
  'body-willchange': '.ff-table__body { will-change: transform }',
  'scroll-isolate': '.ff-table__scroll { isolation: isolate; contain: paint }',
  'body-contain-strict': '.ff-table__body { contain: layout paint style }',
  'no-rows': '.ff-table__scroll { visibility: hidden !important }',
};

const VARIANT_JS: Record<string, string> = {
  'row-img': `for (const c of document.querySelectorAll('[data-track-row] canvas')) {
    const img = new Image();
    img.className = c.className;
    img.style.cssText = c.style.cssText;
    img.width = c.width; img.height = c.height;
    img.src = c.toDataURL();
    c.replaceWith(img);
  }`,
};

test('r118 frame cost', async ({ browser, browserName }) => {
  test.skip(!ENABLED || browserName !== 'chromium' || process.env.FF_PERF_R118 !== 'frames', 'set FF_PERF_R118=frames');
  test.setTimeout(600_000);
  const { context, page } = await openApp(browser);
  await playAndExpand(page, 3);
  let n = 0;
  for (const name of (process.env.FF_PERF_VARIANTS ?? 'base').split(',')) {
    const css = VARIANT_CSS[name] ?? '';
    await page.locator('[data-track-row]').nth(5 + 2 * n).dispatchEvent('dblclick');
    await page.waitForTimeout(1200);
    const handle = css ? await page.addStyleTag({ content: css }) : null;
    if (VARIANT_JS[name]) await page.evaluate(VARIANT_JS[name]);
    await page.waitForTimeout(600);
    await browser.startTracing(page, { path: `${OUT}-${name}-${n++}.json`, categories: TRACE_CATEGORIES });
    await page.waitForTimeout(1500);
    await browser.stopTracing();
    await handle?.evaluate((el) => el.remove());
  }
  await context.close();
  writeFileSync(`${OUT}.done`, '');
});

const settleFrames = (page: Page) => page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(resolve, 150)))));

for (const dpr of (process.env.FF_PERF_DPRS ?? '1,2').split(',').map(Number)) {
  test(`r118 identity dpr${dpr}`, async ({ browser, browserName }) => {
    test.skip(process.env.FF_PERF_R118 !== 'identity', 'set FF_PERF_R118=identity FF_PERF_R118_OUT=dir/prefix');
    test.setTimeout(300_000);
    const { context, page } = await openApp(browser, dpr);
    const table = page.locator('.ff-table__scroll');
    const hashes: Record<string, string> = {};
    const shot = async (name: string) => {
      await settleFrames(page);
      const buffer = await table.screenshot({ animations: 'disabled' });
      writeFileSync(`${OUT}-${browserName}-dpr${dpr}-${name}.png`, buffer);
      hashes[name] = createHash('sha1').update(buffer).digest('hex').slice(0, 12);
    };
    const scrollTo = async (top: number | 'end') => {
      await page.evaluate((t) => {
        const el = document.querySelector('.ff-table__scroll') as HTMLElement;
        el.scrollTop = t === 'end' ? el.scrollHeight : t;
      }, top);
      await page.waitForTimeout(400);
    };
    await shot('top');
    await scrollTo(2800);
    await shot('row100');
    await scrollTo(140_000);
    await shot('row5000');
    await scrollTo('end');
    await shot('end');
    await scrollTo(0);
    for (let top = 0; top <= 777; top += 7) await page.evaluate((t) => ((document.querySelector('.ff-table__scroll') as HTMLElement).scrollTop = t), top);
    await shot('stepped-777');
    await scrollTo(0);
    const rows = page.locator('[data-track-row]');
    await rows.nth(4).locator('.ff-trow__cell--title').hover();
    await shot('hover-row4');
    const wave = rows.nth(6).locator('.ff-trow__cell--wave');
    const box = (await wave.boundingBox())!;
    await page.mouse.move(box.x + box.width * 0.37, box.y + box.height / 2);
    await shot('hover-wave6');
    await rows.nth(2).locator('.ff-trow__cell--title').click();
    await page.mouse.move(5, 5);
    await shot('selected-2');
    await rows.nth(7).locator('.ff-trow__cell--title').click({ modifiers: ['Shift'] });
    await page.mouse.move(5, 5);
    await shot('range-2-7');
    await rows.nth(9).dblclick();
    await page.waitForTimeout(1200);
    await page.mouse.move(5, 5);
    await shot('playing-9');
    await page.keyboard.press('Control+k');
    await page.keyboard.type('Analyze all unanalyzed');
    await page.keyboard.press('Enter');
    await expect(page.locator('.ff-cp')).toHaveCount(0);
    await page.locator('.ff-gheader__search input').fill('');
    await page.keyboard.press('Escape');
    await page.waitForTimeout(600);
    await shot('analysing');
    await scrollTo(2800);
    await shot('analysing-row100');
    writeFileSync(`${OUT}-${browserName}-dpr${dpr}.json`, JSON.stringify(hashes, null, 1));
    console.log(`identity ${browserName} dpr${dpr}: ${JSON.stringify(hashes)}`);
    await context.close();
  });
}

interface FrameStats {
  frames: number;
  p50: number;
  p95: number;
  over20: number;
  worst: number;
}

const stats = (gaps: number[]): FrameStats => {
  const sorted = [...gaps].sort((a, b) => a - b);
  return { frames: gaps.length, p50: sorted[Math.floor(sorted.length / 2)], p95: sorted[Math.floor(sorted.length * 0.95)], over20: gaps.filter((g) => g > 20).length, worst: sorted.at(-1) ?? 0 };
};

test('r118 scroll', async ({ browser, browserName }) => {
  test.skip(process.env.FF_PERF_R118 !== 'scroll' || browserName !== 'chromium', 'set FF_PERF_R118=scroll (chromium traces)');
  test.setTimeout(300_000);
  const { context, page } = await openApp(browser);
  for (let pass = 0; pass < Number(process.env.FF_PERF_PASSES ?? 3); pass++) {
    await page.evaluate(() => ((document.querySelector('.ff-table__scroll') as HTMLElement).scrollTop = 0));
    await page.waitForTimeout(500);
    await browser.startTracing(page, { path: `${OUT}-scroll-${pass}.json`, categories: TRACE_CATEGORIES });
    const gaps = await page.evaluate(
      () =>
        new Promise<number[]>((resolve) => {
          const el = document.querySelector('.ff-table__scroll') as HTMLElement;
          const gaps: number[] = [];
          let last = performance.now();
          let frame = 0;
          const tick = (now: number) => {
            gaps.push(now - last);
            last = now;
            el.scrollTop = 84 * ++frame;
            if (frame < 150) requestAnimationFrame(tick);
            else resolve(gaps);
          };
          requestAnimationFrame(tick);
        }),
    );
    await browser.stopTracing();
    writeFileSync(`${OUT}-scroll-${pass}.stats`, JSON.stringify(stats(gaps.slice(1))));
  }
  await context.close();
});

test('r118 typing', async ({ browser, browserName }) => {
  test.skip(process.env.FF_PERF_R118 !== 'typing' || browserName !== 'chromium', 'set FF_PERF_R118=typing (chromium traces)');
  test.setTimeout(300_000);
  const { context, page } = await openApp(browser);
  const input = page.locator('.ff-gheader__search input');
  await input.click();
  for (let pass = 0; pass < Number(process.env.FF_PERF_PASSES ?? 3); pass++) {
    await input.fill('');
    await page.waitForTimeout(500);
    await browser.startTracing(page, { path: `${OUT}-typing-${pass}.json`, categories: TRACE_CATEGORIES });
    const totals: number[] = [];
    for (const key of 'house'.split('').concat(['Backspace', 'Backspace', 'Backspace', 'Backspace', 'Backspace'])) {
      await page.evaluate(() => ((window as unknown as { __t0: number }).__t0 = performance.now()));
      await page.keyboard.press(key);
      totals.push(await page.evaluate(() => new Promise<number>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve(performance.now() - (window as unknown as { __t0: number }).__t0))))));
      await page.waitForTimeout(120);
    }
    await browser.stopTracing();
    writeFileSync(`${OUT}-typing-${pass}.stats`, JSON.stringify({ totals, ...stats(totals) }));
  }
  await context.close();
});

async function browserRssMb(browser: Browser): Promise<number> {
  const session = await browser.newBrowserCDPSession();
  const { processInfo } = (await session.send('SystemInfo.getProcessInfo')) as { processInfo: { id: number }[] };
  await session.detach();
  const pids = new Set(processInfo.map((p) => p.id));
  let kb = 0;
  for (const line of execSync('ps -eo pid=,rss=').toString().trim().split('\n')) {
    const [pid, rss] = line.trim().split(/\s+/).map(Number);
    if (pids.has(pid)) kb += rss;
  }
  return kb / 1024;
}

test('r118 memory', async ({ browser, browserName }) => {
  test.skip(process.env.FF_PERF_R118 !== 'memory' || browserName !== 'chromium', 'set FF_PERF_R118=memory (chromium)');
  test.setTimeout(300_000);
  const dpr = Number(process.env.FF_PERF_DPR ?? 2);
  const { context, page } = await openApp(browser, dpr);
  const cdp = await context.newCDPSession(page);
  const heap = async () => {
    await cdp.send('HeapProfiler.collectGarbage');
    return (((await cdp.send('Runtime.getHeapUsage')) as { usedSize: number }).usedSize) / 1048576;
  };
  const before = { heapMb: await heap(), rssMb: await browserRssMb(browser) };
  for (let sweep = 0; sweep < 2; sweep++) {
    await page.evaluate(
      () =>
        new Promise<void>((resolve) => {
          const el = document.querySelector('.ff-table__scroll') as HTMLElement;
          let top = 0;
          const tick = () => {
            top += 28 * 20;
            el.scrollTop = top;
            if (top < el.scrollHeight) requestAnimationFrame(tick);
            else resolve();
          };
          tick();
        }),
    );
    await page.waitForTimeout(500);
  }
  await page.waitForTimeout(1500);
  const urls = await page.evaluate(() => {
    const lengths = [...document.querySelectorAll('[data-track-row] img.ff-waveform')].map((img) => (img.getAttribute('src') ?? '').length);
    return { images: lengths.length, meanChars: lengths.length ? lengths.reduce((a, b) => a + b, 0) / lengths.length : 0 };
  });
  const after = { heapMb: await heap(), rssMb: await browserRssMb(browser) };
  await page.waitForTimeout(8000);
  const later = { heapMb: await heap(), rssMb: await browserRssMb(browser) };
  console.log(`memory dpr${dpr} ${JSON.stringify({ before, after, later, urls })}`);
  await context.close();
});

interface InspectorSession {
  send(method: string, params?: Record<string, unknown>): Promise<unknown>;
  on(event: string, listener: (payload: unknown) => void): void;
}

interface TimelineRecord {
  type: string;
  startTime: number;
  endTime: number;
  children?: TimelineRecord[];
}

function webkitInspectorSession(page: Page): InspectorSession {
  const internal = page as unknown as { _connection: { toImpl(x: unknown): { delegate: { _session: InspectorSession } } } };
  return internal._connection.toImpl(page).delegate._session;
}

async function webkitTimeline(page: Page, run: () => Promise<void>): Promise<Record<string, number>> {
  const session = webkitInspectorSession(page);
  const records: TimelineRecord[] = [];
  session.on('Timeline.eventRecorded', (event) => records.push((event as { record: TimelineRecord }).record));
  await session.send('Timeline.enable');
  await session.send('Timeline.start');
  await run();
  await session.send('Timeline.stop');
  await session.send('Timeline.disable');
  const totals: Record<string, number> = {};
  const add = (record: TimelineRecord) => {
    totals[record.type] = (totals[record.type] ?? 0) + (record.endTime - record.startTime) * 1000;
    totals[`n:${record.type}`] = (totals[`n:${record.type}`] ?? 0) + 1;
    record.children?.forEach(add);
  };
  records.forEach(add);
  return totals;
}

test('r118 webkit frames', async ({ browser, browserName }) => {
  test.skip(process.env.FF_PERF_R118 !== 'webkit-frames' || browserName !== 'webkit', 'set FF_PERF_R118=webkit-frames FF_SMOKE_BROWSER=webkit');
  test.setTimeout(600_000);
  const { context, page } = await openApp(browser);
  const scenario = process.env.FF_PERF_SCENARIO ?? 'expanded';
  if (scenario === 'expanded') await playAndExpand(page, 3);
  for (let pass = 0; pass < Number(process.env.FF_PERF_PASSES ?? 3); pass++) {
    let gaps: number[] = [];
    const totals = await webkitTimeline(page, async () => {
      gaps = await page.evaluate(
        (scenario) =>
          new Promise<number[]>((resolve) => {
            const el = document.querySelector('.ff-table__scroll') as HTMLElement;
            if (scenario === 'scroll') el.scrollTop = 0;
            const gaps: number[] = [];
            let last = performance.now();
            let frame = 0;
            const tick = (now: number) => {
              gaps.push(now - last);
              last = now;
              frame++;
              if (scenario === 'scroll') el.scrollTop = 84 * frame;
              if (frame < (scenario === 'scroll' ? 90 : 60)) requestAnimationFrame(tick);
              else resolve(gaps);
            };
            requestAnimationFrame(tick);
          }),
        scenario,
      );
    });
    const frameStats = stats(gaps.slice(1));
    writeFileSync(`${OUT}-wk-${scenario}-${pass}.json`, JSON.stringify({ totals, ...frameStats, ms: gaps.slice(1).reduce((a, b) => a + b, 0) }));
    await page.waitForTimeout(400);
  }
  await context.close();
});

test('r118 pool', async ({ browser }) => {
  test.skip(process.env.FF_PERF_R118 !== 'pool', 'set FF_PERF_R118=pool');
  const { context, page } = await openApp(browser);
  const counts = await page.evaluate(
    () =>
      new Promise<{ rows: number; waves: number; addedWaves: number; removedWaves: number; addedRows: number }>((resolve) => {
        const el = document.querySelector('.ff-table__scroll') as HTMLElement;
        const waves = () => document.querySelectorAll('[data-track-row] .ff-waveform').length;
        const result = { rows: document.querySelectorAll('[data-track-row]').length, waves: waves(), addedWaves: 0, removedWaves: 0, addedRows: 0 };
        const isWave = (n: Node) => n instanceof Element && (n.classList.contains('ff-waveform') || !!n.querySelector('.ff-waveform'));
        const observer = new MutationObserver((records) => {
          for (const r of records) {
            r.addedNodes.forEach((n) => {
              if (isWave(n)) result.addedWaves++;
              if (n instanceof Element && n.hasAttribute('data-track-row')) result.addedRows++;
            });
            r.removedNodes.forEach((n) => isWave(n) && result.removedWaves++);
          }
        });
        observer.observe(el, { childList: true, subtree: true });
        let frame = 0;
        const tick = () => {
          el.scrollTop = 84 * ++frame;
          if (frame < 300) requestAnimationFrame(tick);
          else setTimeout(() => (observer.disconnect(), resolve(result)), 300);
        };
        requestAnimationFrame(tick);
      }),
  );
  console.log(`pool ${JSON.stringify(counts)}`);
  await context.close();
});
