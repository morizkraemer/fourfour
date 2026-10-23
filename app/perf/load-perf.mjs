import { createRequire } from 'node:module';

const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [base, count = '10000', variant = ''] = process.argv.slice(2);
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const client = await page.context().newCDPSession(page);
const errors = [];
page.on('pageerror', (e) => errors.push(e.message));

await page.goto(`${base}?mock=${count}${variant ? `&${variant}` : ''}`, { waitUntil: 'commit', timeout: 0 });
await page.waitForSelector('[data-track-row]', { timeout: 900000 });
await page.waitForTimeout(2500);

function trial(action, mark, rowIndex) {
  return page.evaluate(
    ({ action, mark, rowIndex }) =>
      new Promise((resolve) => {
        performance.clearMarks(mark);
        const row = document.querySelectorAll('[data-track-row]')[rowIndex];
        const t0 = performance.now();
        let longTaskMs = 0;
        const observer = new PerformanceObserver((list) => {
          for (const entry of list.getEntries()) longTaskMs += entry.duration;
        });
        observer.observe({ type: 'longtask', buffered: false });
        row.dispatchEvent(new MouseEvent(action, { bubbles: true, cancelable: true }));
        const poll = () => {
          const found = performance.getEntriesByName(mark).at(-1);
          if (!found) return setTimeout(poll, 0);
          requestAnimationFrame(() =>
            requestAnimationFrame(() => {
              observer.disconnect();
              resolve({ ready: found.startTime - t0, painted: performance.now() - t0, longTaskMs });
            }),
          );
        };
        poll();
      }),
    { action, mark, rowIndex },
  );
}

const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];
const round = (n) => Math.round(n * 10) / 10;

const select = [];
for (const idx of [3, 5, 2, 8, 6]) {
  select.push(await trial('click', 'ff:detail-ready', idx));
  await page.waitForTimeout(300);
}
const load = [];
for (const idx of [13, 15, 12, 18, 16]) {
  load.push(await trial('dblclick', 'ff:player-ready', idx));
  await page.waitForTimeout(300);
}

const expand = await page.evaluate(
  () =>
    new Promise((resolve) => {
      const t0 = performance.now();
      document.querySelector('.ff-player-compact__expand-btn').click();
      requestAnimationFrame(() => requestAnimationFrame(() => resolve(performance.now() - t0)));
    }),
);

const metrics = Object.fromEntries((await client.send('Performance.getMetrics')).metrics.map((m) => [m.name, m.value]));
const summarise = (runs) => ({
  ready: round(median(runs.map((r) => r.ready))),
  painted: round(median(runs.map((r) => r.painted))),
  longTask: round(median(runs.map((r) => r.longTaskMs))),
  all: runs.map((r) => round(r.painted)),
});
console.log(
  JSON.stringify({
    count: Number(count),
    variant,
    selectToDetail: summarise(select),
    loadToWaveform: summarise(load),
    expandMs: round(expand),
    heapMb: round(metrics.JSHeapUsedSize / 1048576),
    errors,
  }),
);
await browser.close();
