import { createRequire } from 'node:module';

const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [base = 'http://127.0.0.1:4310/', count = '1000'] = process.argv.slice(2);
const url = `${base}?mock=${count}`;

const pct = (arr, p) => {
  if (!arr.length) return 0;
  const s = [...arr].sort((a, b) => a - b);
  return s[Math.min(s.length - 1, Math.floor((p / 100) * s.length))];
};
const round = (n) => Math.round(n * 10) / 10;

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const errors = [];
page.on('console', (m) => {
  if (m.type() === 'error') errors.push(`console: ${m.text()}`);
});
page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));

await page.addInitScript(() => {
  window.__perf = { firstRow: null, longTasks: [], rowCountAtFirst: 0 };
  new PerformanceObserver((list) => {
    for (const e of list.getEntries()) window.__perf.longTasks.push([e.startTime, e.duration]);
  }).observe({ entryTypes: ['longtask'] });
  const mo = new MutationObserver(() => {
    if (window.__perf.firstRow === null && document.querySelector('[data-track-row]')) {
      window.__perf.firstRow = performance.now();
    }
  });
  mo.observe(document, { childList: true, subtree: true });
});

const client = await page.context().newCDPSession(page);
await client.send('Performance.enable');

await page.goto(url, { waitUntil: 'commit', timeout: 0 });
await page.waitForSelector('[data-track-row]', { timeout: 900000 });
await page.waitForFunction(() => window.__perf.longTasks.length === 0 || performance.now() - (window.__perf.longTasks.at(-1)[0] + window.__perf.longTasks.at(-1)[1]) > 300, null, { timeout: 900000 });

const load = await page.evaluate(() => {
  const nav = performance.getEntriesByType('navigation')[0];
  const fcp = performance.getEntriesByName('first-contentful-paint')[0]?.startTime ?? null;
  const lastLong = window.__perf.longTasks.at(-1);
  return {
    fcp,
    domContentLoaded: nav.domContentLoadedEventEnd,
    firstRowMs: window.__perf.firstRow,
    interactiveMs: lastLong ? lastLong[0] + lastLong[1] : window.__perf.firstRow,
    totalBlockingMs: window.__perf.longTasks.reduce((a, [, d]) => a + Math.max(0, d - 50), 0),
    rowsInDom: document.querySelectorAll('[data-track-row]').length,
    domNodes: document.getElementsByTagName('*').length,
  };
});
const metrics = Object.fromEntries((await client.send('Performance.getMetrics')).metrics.map((m) => [m.name, m.value]));
const heapMb = metrics.JSHeapUsedSize / 1048576;

const scroll = await page.evaluate(
  () =>
    new Promise((resolve) => {
      const el = document.querySelector('.ff-table__scroll');
      const deltas = [];
      let last = performance.now();
      const start = last;
      const tick = (now) => {
        deltas.push(now - last);
        last = now;
        el.scrollTop += 90;
        if (now - start < 3000) requestAnimationFrame(tick);
        else resolve({ deltas, travelled: el.scrollTop });
      };
      requestAnimationFrame(tick);
    }),
);
await page.evaluate(() => (document.querySelector('.ff-table__scroll').scrollTop = 0));
await page.waitForTimeout(300);

const searchInput = page.locator('input[type="search"], input[placeholder*="earch" i], input[placeholder*="ilter" i]').first();
await searchInput.click();
const searchTimes = [];
await page.exposeFunction('__report', (n) => searchTimes.push(n));
await page.evaluate(() => {
  document.addEventListener(
    'keydown',
    () => {
      const t0 = performance.now();
      setTimeout(() => {
        requestAnimationFrame(() => requestAnimationFrame(() => window.__report(performance.now() - t0)));
      }, 0);
    },
    true,
  );
});
for (const ch of 'track 12') {
  await page.keyboard.press(ch === ' ' ? 'Space' : ch);
  await page.waitForTimeout(450);
}
await page.keyboard.press('Escape');
await page.waitForTimeout(400);
await page.evaluate(() => (document.activeElement instanceof HTMLElement ? document.activeElement.blur() : null));

const selectTimes = [];
for (const idx of [3, 5, 2, 8]) {
  const t = await page.evaluate(
    (i) =>
      new Promise((resolve) => {
        const row = (() => { const r = document.querySelectorAll("[data-track-row]"); return r[Math.min(i, r.length - 1)]; })();
        const t0 = performance.now();
        row.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
        requestAnimationFrame(() => requestAnimationFrame(() => resolve(performance.now() - t0)));
      }),
    idx,
  );
  selectTimes.push(t);
  await page.waitForTimeout(200);
}

const shiftTime = await page.evaluate(
  () =>
    new Promise((resolve) => {
      const row = (() => { const r = document.querySelectorAll("[data-track-row]"); return r[Math.min(20, r.length - 1)]; })();
      const t0 = performance.now();
      row.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, shiftKey: true }));
      requestAnimationFrame(() => requestAnimationFrame(() => resolve(performance.now() - t0)));
    }),
);

const frames = scroll.deltas.slice(2);
const result = {
  tracks: Number(count),
  fcpMs: round(load.fcp ?? -1),
  firstRowMs: round(load.firstRowMs),
  interactiveMs: round(load.interactiveMs),
  totalBlockingMs: round(load.totalBlockingMs),
  rowsInDom: load.rowsInDom,
  domNodes: load.domNodes,
  heapMb: round(heapMb),
  scrollFrameP50: round(pct(frames, 50)),
  scrollFrameP95: round(pct(frames, 95)),
  scrollFrameMax: round(Math.max(...frames)),
  scrollFramesOver33ms: frames.filter((d) => d > 33).length,
  scrollFrames: frames.length,
  scrollTravelledPx: Math.round(scroll.travelled),
  searchKeystrokeMs: searchTimes.map(round),
  selectClickMs: selectTimes.map(round),
  shiftRangeClickMs: round(shiftTime),
  errors,
};
console.log(JSON.stringify(result));
await browser.close();
