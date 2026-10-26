import { createRequire } from 'node:module';
import { mkdirSync } from 'node:fs';

const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [base, outDir, theme = 'dark', dsf = '1', count = '3000'] = process.argv.slice(2);
mkdirSync(outDir, { recursive: true });

const LIGHT_BASE_TOKENS = `:root[data-theme='light']{
  --ff-bg:#f4f5f7;--ff-surface:#ffffff;--ff-elev:#eceef1;--ff-elev-hi:#e3e6ea;
  --ff-text:#16181d;--ff-text-mid:#4b515c;--ff-muted:#7b8290;--ff-faint:#a6adb8;
  --ff-border:rgba(0,0,0,.08);--ff-border-hi:rgba(0,0,0,.14);--ff-hover:rgba(0,0,0,.04);
  --ff-select:rgba(0,0,0,.07);--ff-accent:#111111;--ff-accent-hi:#000000;color-scheme:light}
  :root[data-theme='light'] body{background:var(--ff-bg);color:var(--ff-text)}`;

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
if (theme === 'light') {
  await page.addStyleTag({ content: LIGHT_BASE_TOKENS });
  await page.evaluate(() => (document.documentElement.dataset.theme = 'light'));
}
await page.waitForTimeout(2500);

const settle = (ms = 1200) => page.waitForTimeout(ms);
async function snap(name, selector) {
  await page.screenshot({ path: `${outDir}/${name}.png` });
  const box = await page.locator(selector).first().boundingBox();
  await page.screenshot({
    path: `${outDir}/${name}-crop.png`,
    clip: { x: Math.floor(box.x), y: Math.floor(box.y), width: Math.ceil(box.width), height: Math.ceil(box.height) },
  });
}

await snap('list', '.ff-table__scroll');

await page.locator('[data-track-row]').nth(4).click();
await settle();
await snap('detail', '.ff-detail-pane');

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
await snap('player-compact', '.ff-player-compact');

await page.locator('.ff-player-compact__expand-btn').click();
await settle(1500);
const overview = await page.locator('.waveform-display__overview').boundingBox();
await page.mouse.click(overview.x + overview.width * 0.3713, overview.y + overview.height / 2);
await settle(1200);
await snap('player-expanded', '.ff-xp__wave');

await page.locator('[aria-label="Zoom in"]').click();
await page.locator('[aria-label="Zoom in"]').click();
await settle();
await snap('player-expanded-zoomed', '.ff-xp__wave');

await page.locator('[aria-label="Reset zoom"]').click();
await settle();
await snap('player-expanded-fit', '.ff-xp__wave');

console.log(JSON.stringify({ outDir, theme, dsf, errors }));
await browser.close();
