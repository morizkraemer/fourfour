import { createRequire } from 'node:module';
import { mkdirSync } from 'node:fs';

const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [base, outDir, dsf = '1'] = process.argv.slice(2);
mkdirSync(outDir, { recursive: true });

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: Number(dsf) });
const errors = [];
page.on('console', (m) => {
  if (m.type() === 'error') errors.push(`console: ${m.text()}`);
});
page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));

await page.goto(`${base}?mock=2000&artwork=4`, { waitUntil: 'commit', timeout: 0 });
await page.waitForSelector('[data-track-row]', { timeout: 900000 });
await page.addStyleTag({ content: '*,*::before,*::after{animation:none!important;transition:none!important;caret-color:transparent!important}' });
await page.waitForTimeout(3000);

const snap = (name) => page.screenshot({ path: `${outDir}/${name}.png` });
await snap('covers');

const row = page.locator('[data-track-row]').nth(6);
const box = await row.boundingBox();
await page.mouse.move(box.x + 300, box.y + 10);
await page.mouse.down();
await page.mouse.move(box.x + 330, box.y + 60, { steps: 5 });
await page.mouse.move(box.x + 420, box.y + 170, { steps: 8 });
await page.waitForTimeout(400);
await snap('drag-ghost');
await page.mouse.up();
await page.waitForTimeout(300);

console.log(errors.length ? errors.join('\n') : 'no errors');
await browser.close();
