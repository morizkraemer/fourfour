import { createRequire } from 'node:module';
import { mkdirSync, readFileSync, readdirSync } from 'node:fs';

const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [base, fixtureDir, outDir, dsf = '2'] = process.argv.slice(2);
mkdirSync(outDir, { recursive: true });

const browser = await chromium.launch();
const errors = [];
const files = readdirSync(fixtureDir).filter((f) => f.endsWith('.json')).sort();

for (const file of files) {
  const page = await browser.newPage({ viewport: { width: 1160, height: 900 }, deviceScaleFactor: Number(dsf) });
  page.on('console', (m) => {
    if (m.type() === 'error') errors.push(`${file} console: ${m.text()}`);
  });
  page.on('pageerror', (e) => errors.push(`${file} pageerror: ${e.message}`));
  const body = readFileSync(`${fixtureDir}/${file}`);
  await page.route('**/compare-fixture.json', (route) => route.fulfill({ contentType: 'application/json', body }));
  await page.goto(`${base}/compare.html${process.env.COMPARE_QUERY ?? ""}`, { waitUntil: 'load' });
  await page.waitForSelector('body[data-ready="1"]', { timeout: 60000 });
  await page.addStyleTag({ content: '*,*::before,*::after{animation:none!important;transition:none!important}' });
  await page.waitForTimeout(1800);
  const box = await page.locator('#compare').boundingBox();
  const out = `${outDir}/${file.replace('.json', '.png')}`;
  await page.screenshot({ path: out, clip: { x: 0, y: 0, width: 1160, height: Math.ceil(box.height) + 8 } });
  console.log(out);
  await page.close();
}
await browser.close();
if (errors.length) {
  console.log(errors.join('\n'));
  process.exitCode = 1;
}
