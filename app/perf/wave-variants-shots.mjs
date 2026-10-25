import { createRequire } from 'node:module';
import { mkdirSync } from 'node:fs';

const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [base, outDir, dsf = '2', count = '200'] = process.argv.slice(2);
mkdirSync(outDir, { recursive: true });

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: Number(dsf) });
const errors = [];
page.on('console', (m) => {
  if (m.type() === 'error') errors.push(`console: ${m.text()}`);
});
page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));

async function crop(name, selector) {
  const box = await page.locator(selector).first().boundingBox();
  await page.screenshot({
    path: `${outDir}/${name}.png`,
    clip: { x: Math.floor(box.x), y: Math.floor(box.y), width: Math.ceil(box.width), height: Math.ceil(box.height) },
  });
}

async function open(url) {
  await page.goto(url, { waitUntil: 'commit', timeout: 0 });
  await page.waitForSelector('[data-track-row]', { timeout: 120000 });
  await page.addStyleTag({ content: '*,*::before,*::after{animation:none!important;transition:none!important;caret-color:transparent!important}' });
  await page.waitForTimeout(1500);
}

await open(`${base}?mock=${count}`);
await page.locator('[data-track-row]').nth(2).click();
await page.waitForTimeout(800);
await page.screenshot({ path: `${outDir}/baseline-no-param-full.png` });
const legendWithoutParam = await page.locator('[data-wave-legend]').count();
console.log('legend without param:', legendWithoutParam);

for (const variant of ['a', 'b', 'c', 'd']) {
  await open(`${base}?mock=${count}&wave=${variant}`);
  await page.locator('[data-track-row]').nth(2).click();
  await page.waitForTimeout(800);
  await crop(`${variant}-list`, '.ff-table__scroll');
  await crop(`${variant}-detail`, '.ff-detail-pane');
  await page.screenshot({ path: `${outDir}/${variant}-full.png` });
}

await open(`${base}?mock=${count}&wave=a`);
await page.locator('[data-track-row]').nth(2).click();
for (const [key, variant] of [['2', 'b'], ['3', 'c'], ['4', 'd'], ['1', 'a']]) {
  await page.keyboard.press(key);
  await page.waitForTimeout(600);
  const title = await page.locator('.wave-legend__title').textContent();
  console.log(`key ${key} ->`, title, title === `wave: ${variant}` ? 'ok' : 'WRONG');
}

console.log(errors.length ? errors.join('\n') : 'no console errors');
await browser.close();
