import { createRequire } from 'node:module';
import { mkdirSync } from 'node:fs';

const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [base, outDir, count = '30000', dsf = '1'] = process.argv.slice(2);
mkdirSync(outDir, { recursive: true });

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: Number(dsf) });
const errors = [];
page.on('console', (m) => {
  if (m.type() === 'error') errors.push(`console: ${m.text()}`);
});
page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));

await page.goto(`${base}?mock=${count}`, { waitUntil: 'commit', timeout: 0 });
await page.waitForSelector('[data-track-row]', { timeout: 900000 });
await page.addStyleTag({ content: '*,*::before,*::after{animation:none!important;transition:none!important;caret-color:transparent!important}' });
await page.waitForTimeout(2500);

const snap = (name) => page.screenshot({ path: `${outDir}/${name}.png` });
const settle = (ms = 1200) => page.waitForTimeout(ms);
const scroller = '.ff-table__scroll';
const setTop = (v) => page.evaluate(([s, v]) => (document.querySelector(s).scrollTop = v), [scroller, v]);

await snap('list-top');

await setTop(123456);
await settle();
await snap('list-mid');

await setTop(1e9);
await settle();
await snap('list-end');

for (let i = 0; i < 40; i++) {
  await page.evaluate((s) => (document.querySelector(s).scrollTop += 90), scroller);
  await page.waitForTimeout(16);
}
await setTop(7777);
await settle();
await snap('list-after-scroll');

await setTop(0);
await settle();

await page.evaluate(
  () =>
    new Promise((resolve) => {
      document.querySelectorAll('[data-track-row]')[13].dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true }));
      const poll = () => (performance.getEntriesByName('ff:player-ready').length ? resolve() : setTimeout(poll, 0));
      poll();
    }),
);
await settle(1500);
const label = await page.locator('.ff-player-compact__play-btn').getAttribute('aria-label');
if (label === 'Pause') await page.locator('.ff-player-compact__play-btn').click();
await settle(500);

const wave = await page.locator('.ff-player-compact .ff-waveform-view').boundingBox();
await page.mouse.click(wave.x + wave.width * 0.4, wave.y + wave.height / 2);
await settle(1500);
await snap('player-compact');

for (const [i, f] of [0.1234, 0.3713, 0.8137].entries()) {
  await page.mouse.click(wave.x + wave.width * f, wave.y + wave.height / 2);
  await settle(900);
  await snap(`player-compact-seek-${i}`);
}

await page.locator('.ff-player-compact__expand-btn').click();
await settle(1500);
await snap('player-expanded');

const overview = await page.locator('.waveform-display__overview').boundingBox();
for (const [i, f] of [0.1234, 0.3713, 0.8137].entries()) {
  await page.mouse.click(overview.x + overview.width * f, overview.y + overview.height / 2);
  await settle(900);
  await snap(`player-expanded-seek-${i}`);
}

await page.locator('[aria-label="Zoom in"]').click();
await page.locator('[aria-label="Zoom in"]').click();
await settle();
await snap('player-expanded-zoomed');

await page.locator('[aria-label="Reset zoom"]').click();
await settle();
await snap('player-expanded-fit');

console.log(JSON.stringify({ outDir, errors }));
await browser.close();
