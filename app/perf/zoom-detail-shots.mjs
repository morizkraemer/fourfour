import { createRequire } from 'node:module';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';

const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [base, fixtures, out, label, mode = 'shots'] = process.argv.slice(2);
mkdirSync(out, { recursive: true });

const tracks = ['techno-process__original', 'vocal-fata-morgana__original', 'pads-ocean-of-methane__original'];
const windows = [8, 3, 1];
const at = 12;

const browser = await chromium.launch();
const context = await browser.newContext({ viewport: { width: 1200, height: 300 }, deviceScaleFactor: 2 });
await context.route('**/fx/*', (route) => {
  const file = new URL(route.request().url()).pathname.replace('/fx/', '');
  route.fulfill({ body: readFileSync(`${fixtures}/${file}`), contentType: 'application/octet-stream' });
});
await context.route(/^https?:\/\/(?!localhost)/, (route) => route.abort());
const page = await context.newPage();
const errors = [];
page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
page.on('pageerror', (e) => errors.push(String(e)));

const results = {};
for (const track of tracks) {
  for (const win of windows) {
    await page.goto(`${base}/zoom-detail.html?track=${track}&win=${win}&at=${at}`);
    await page.waitForSelector('body[data-ready="1"]');
    await page.waitForTimeout(300);
    if (mode === 'shots') {
      await page.screenshot({ path: `${out}/${label}-${track.split("__")[0]}-${win}s.png`, clip: { x: 0, y: 0, width: 1128, height: 220 } });
    } else {
      const runs = [];
      for (let i = 0; i < 5; i++) runs.push(await page.evaluate(() => window.__bench(300, 1)));
      runs.sort((a, b) => a.mean - b.mean);
      results[`${track.split('__')[0]} ${win}s`] = runs[2];
    }
  }
}
if (mode !== 'shots') {
  writeFileSync(`${out}/${label}-frametimes.json`, JSON.stringify(results, null, 2));
  console.log(JSON.stringify(results, null, 2));
}
if (errors.length) console.log('console errors:', errors);
await browser.close();
