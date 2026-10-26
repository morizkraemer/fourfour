import { createRequire } from 'node:module';

const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [base = 'http://127.0.0.1:4332/', count = '30000', runs = '5', idleMs = '4000'] = process.argv.slice(2);
const url = `${base}?mock=${count}`;
const round = (n) => Math.round(n * 10) / 10;
const median = (a) => [...a].sort((x, y) => x - y)[Math.floor(a.length / 2)];

const results = [];
for (let r = 0; r < Number(runs); r++) {
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  const client = await page.context().newCDPSession(page);
  await client.send('Performance.enable');
  await page.goto(url, { waitUntil: 'commit', timeout: 0 });
  await page.waitForSelector('[data-track-row]', { timeout: 900000 });
  await page.waitForTimeout(Number(idleMs));
  const input = page.locator('input[placeholder*="earch" i]').first();
  await input.click();
  await page.evaluate(() => {
    window.__times = [];
    document.addEventListener(
      'keydown',
      () => {
        const t0 = performance.now();
        setTimeout(() => requestAnimationFrame(() => requestAnimationFrame(() => window.__times.push(performance.now() - t0))), 0);
      },
      true,
    );
  });
  const cpu = async () => {
    const m = Object.fromEntries((await client.send('Performance.getMetrics')).metrics.map((x) => [x.name, x.value]));
    return m.TaskDuration * 1000;
  };
  const cpuMs = [];
  for (const ch of 'track 12') {
    const c0 = await cpu();
    await page.keyboard.press(ch === ' ' ? 'Space' : ch);
    await page.waitForTimeout(600);
    cpuMs.push((await cpu()) - c0);
  }
  const times = await page.evaluate(() => window.__times);
  results.push({ first: times[0], later: median(times.slice(1)), cpuFirst: cpuMs[0], cpuLater: median(cpuMs.slice(1)) });
  await browser.close();
}
const col = (k) => results.map((x) => round(x[k]));
console.log(
  JSON.stringify({
    tracks: Number(count),
    firstMs: col('first'),
    laterMs: col('later'),
    cpuFirstMs: col('cpuFirst'),
    cpuLaterMs: col('cpuLater'),
    median: {
      firstMs: round(median(results.map((x) => x.first))),
      laterMs: round(median(results.map((x) => x.later))),
      cpuFirstMs: round(median(results.map((x) => x.cpuFirst))),
      cpuLaterMs: round(median(results.map((x) => x.cpuLater))),
    },
  }),
);
