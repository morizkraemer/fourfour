import { createRequire } from 'node:module';

const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [base, count = '30000', action = 'scroll'] = process.argv.slice(2);
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
await page.goto(`${base}?mock=${count}`, { waitUntil: 'commit', timeout: 0 });
await page.waitForSelector('[data-track-row]', { timeout: 0 });
await page.waitForTimeout(3000);
if (process.env.INJECT_CSS) await page.addStyleTag({ content: process.env.INJECT_CSS });
if (process.env.INJECT_JS) await page.evaluate(process.env.INJECT_JS);

if (action === 'playing' || action === 'expanded') {
  await page.evaluate(() => document.querySelectorAll('[data-track-row]')[13].dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true })));
  await page.waitForTimeout(1500);
  if ((await page.locator('.ff-player-compact__play-btn').getAttribute('aria-label')) !== 'Pause') {
    await page.locator('.ff-player-compact__play-btn').click();
  }
  if (action === 'expanded') await page.locator('.ff-player-compact__expand-btn').click();
  await page.waitForTimeout(800);
}

await browser.startTracing(undefined, { categories: ['devtools.timeline', 'disabled-by-default-devtools.timeline', 'v8.execute'] });
if (action === 'fling') {
  await page.evaluate(
    () =>
      new Promise((resolve) => {
        const el = document.querySelector('.ff-table__scroll');
        let i = 0;
        const tick = () => {
          el.scrollTop = (el.scrollHeight - el.clientHeight) * ((i + 1) / 20);
          if (++i < 20) requestAnimationFrame(tick);
          else requestAnimationFrame(() => requestAnimationFrame(resolve));
        };
        requestAnimationFrame(tick);
      }),
  );
} else if (action === 'scroll') {
  await page.evaluate(
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
  );
} else {
  await page.waitForTimeout(3000);
}
const buf = await browser.stopTracing();
const trace = JSON.parse(buf.toString());
const events = (trace.traceEvents ?? trace).filter((e) => e.ph === 'X' && e.dur);
const rendererMain = new Map();
for (const e of events) {
  if (e.name === 'RunTask') rendererMain.set(`${e.pid}:${e.tid}`, (rendererMain.get(`${e.pid}:${e.tid}`) ?? 0) + e.dur);
}
const [mainKey] = [...rendererMain.entries()].sort((a, b) => b[1] - a[1])[0];
const main = events.filter((e) => `${e.pid}:${e.tid}` === mainKey).sort((a, b) => a.ts - b.ts || b.dur - a.dur);

const self = new Map();
const stack = [];
for (const e of main) {
  while (stack.length && stack.at(-1).ts + stack.at(-1).dur <= e.ts) stack.pop();
  if (stack.length) stack.at(-1).childDur += e.dur;
  const node = { ...e, childDur: 0 };
  stack.push(node);
  node.self = () => node.dur - node.childDur;
  e.node = node;
}
for (const e of main) {
  const key = e.name === 'FunctionCall' || e.name === 'EvaluateScript' ? `${e.name}` : e.name;
  self.set(key, (self.get(key) ?? 0) + (e.dur - e.node.childDur));
}
const total = [...self.values()].reduce((a, b) => a + b, 0);
console.log(`main thread self time (ms) total ${(total / 1000).toFixed(0)}`);
for (const [k, v] of [...self.entries()].sort((a, b) => b[1] - a[1]).slice(0, 16)) {
  console.log((v / 1000).toFixed(1).padStart(9), k);
}
const stats = new Map();
for (const e of main) {
  const st = stats.get(e.name) ?? { n: 0, total: 0, max: 0 };
  st.n++;
  st.total += e.dur;
  st.max = Math.max(st.max, e.dur);
  stats.set(e.name, st);
}
for (const name of ['Paint', 'PrePaint', 'Layerize', 'Layout', 'UpdateLayoutTree', 'FireAnimationFrame', 'Commit']) {
  const st = stats.get(name);
  if (st) console.log(name.padEnd(18), 'n', st.n, 'avg', (st.total / st.n / 1000).toFixed(2), 'max', (st.max / 1000).toFixed(2));
}
await browser.close();
