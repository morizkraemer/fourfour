import { createRequire } from 'node:module';

const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [base, count, action = 'select'] = process.argv.slice(2);
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const client = await page.context().newCDPSession(page);
await client.send('Profiler.enable');
await client.send('Profiler.setSamplingInterval', { interval: 200 });
if (action === 'load') await client.send('Profiler.start');
await page.goto(`${base}?mock=${count}`, { waitUntil: 'commit', timeout: 0 });
await page.waitForSelector('[data-track-row]', { timeout: 0 });
await page.waitForTimeout(3000);

const actions = {
  select: () =>
    page.evaluate(
      () =>
        new Promise((resolve) => {
          const row = document.querySelectorAll('[data-track-row]')[5];
          row.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
          requestAnimationFrame(() => requestAnimationFrame(resolve));
        }),
    ),
  scroll: () =>
    page.evaluate(
      () =>
        new Promise((resolve) => {
          const el = document.querySelector('.ff-table__scroll');
          const start = performance.now();
          const tick = (now) => {
            el.scrollTop += 90;
            if (now - start < 2000) requestAnimationFrame(tick);
            else resolve();
          };
          requestAnimationFrame(tick);
        }),
    ),
  fling: () =>
    page.evaluate(
      () =>
        new Promise((resolve) => {
          const el = document.querySelector('.ff-table__scroll');
          const start = performance.now();
          const tick = (now) => {
            el.scrollTop = (el.scrollHeight - el.clientHeight) * Math.min(1, (now - start) / 1500);
            if (now - start < 1800) requestAnimationFrame(tick);
            else resolve();
          };
          requestAnimationFrame(tick);
        }),
    ),
  player: () =>
    page.evaluate(
      () =>
        new Promise((resolve) => {
          const row = document.querySelectorAll('[data-track-row]')[13];
          row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true }));
          const poll = () =>
            performance.getEntriesByName('ff:player-ready').length
              ? requestAnimationFrame(() => requestAnimationFrame(resolve))
              : setTimeout(poll, 0);
          poll();
        }),
    ),
  expand: async () => {
    await page.evaluate(
      () =>
        new Promise((resolve) => {
          document.querySelectorAll('[data-track-row]')[13].dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true }));
          const poll = () =>
            performance.getEntriesByName('ff:player-ready').length ? resolve() : setTimeout(poll, 0);
          poll();
        }),
    );
    await page.waitForTimeout(500);
    await client.send('Profiler.stop');
    await client.send('Profiler.start');
    await page.evaluate(
      () =>
        new Promise((resolve) => {
          document.querySelector('.ff-player-compact__expand-btn').click();
          requestAnimationFrame(() => requestAnimationFrame(resolve));
        }),
    );
  },
  playing: async () => {
    await page.evaluate(() => document.querySelectorAll('[data-track-row]')[13].dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true })));
    await page.waitForTimeout(1500);
    if ((await page.locator('.ff-player-compact__play-btn').getAttribute('aria-label')) !== 'Pause') {
      await page.locator('.ff-player-compact__play-btn').click();
    }
    await page.waitForTimeout(800);
    await client.send('Profiler.stop');
    await client.send('Profiler.start');
    await page.waitForTimeout(4000);
  },
  playingExpanded: async () => {
    await page.evaluate(() => document.querySelectorAll('[data-track-row]')[13].dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true })));
    await page.waitForTimeout(1500);
    if ((await page.locator('.ff-player-compact__play-btn').getAttribute('aria-label')) !== 'Pause') {
      await page.locator('.ff-player-compact__play-btn').click();
    }
    await page.locator('.ff-player-compact__expand-btn').click();
    await page.waitForTimeout(800);
    await client.send('Profiler.stop');
    await client.send('Profiler.start');
    await page.waitForTimeout(4000);
  },
  search: async () => {
    await page.locator('input').first().click();
    await page.keyboard.type('track 12', { delay: 150 });
    await page.waitForTimeout(500);
  },
};

if (action !== 'load') {
  await client.send('Profiler.start');
  await actions[action]();
}
const { profile } = await client.send('Profiler.stop');

const self = new Map();
const byId = new Map(profile.nodes.map((n) => [n.id, n]));
const dt = profile.timeDeltas;
profile.samples.forEach((id, i) => {
  const n = byId.get(id);
  const f = n.callFrame;
  const key = `${f.functionName || '(anon)'} ${f.url.split('/').pop()}:${f.lineNumber}`;
  self.set(key, (self.get(key) ?? 0) + dt[i] / 1000);
});
const top = [...self.entries()].sort((a, b) => b[1] - a[1]).slice(0, 18);
for (const [k, v] of top) console.log(v.toFixed(1).padStart(9), 'ms', k);
await browser.close();
