import { createRequire } from 'node:module';
import { mkdirSync } from 'node:fs';

const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [base = 'http://127.0.0.1:4417/', outDir = './wo-shots'] = process.argv.slice(2);
mkdirSync(outDir, { recursive: true });

const browser = await chromium.launch(process.env.CHROME ? { executablePath: process.env.CHROME } : {});
const results = [];

async function openApp(width = 1440) {
  const ctx = await browser.newContext({ viewport: { width, height: 900 } });
  const page = await ctx.newPage();
  await page.goto(`${base}?mock=200`, { waitUntil: 'commit', timeout: 0 });
  await page.waitForSelector('[data-track-row]', { timeout: 60000 });
  await page.waitForTimeout(1200);
  return page;
}

async function dragTrackTo(page, selector, shotName) {
  const row = page.locator('[data-track-row]').nth(3);
  await row.click();
  const src = await row.boundingBox();
  const dst = await page.locator(selector).first().boundingBox();
  await page.mouse.move(src.x + 300, src.y + src.height / 2);
  await page.mouse.down();
  await page.mouse.move(src.x + 280, src.y + 40, { steps: 5 });
  await page.mouse.move(dst.x + dst.width / 2, dst.y + dst.height / 2, { steps: 15 });
  await page.waitForTimeout(250);
  const hovered = await page.evaluate(
    () => !!document.querySelector('.ff-dcard--drop, .ff-sidebar-section__add-btn--drop, .ff-sidebar__fill.drop-into')
  );
  await page.screenshot({ path: `${outDir}/${shotName}-hover.png` });
  await page.mouse.up();
  await page.waitForTimeout(600);
  await page.screenshot({ path: `${outDir}/${shotName}-after.png` });
  return hovered;
}

const playlists = (page) =>
  page.evaluate(() => document.querySelectorAll('.ff-sidebar__drop-wrap[data-playlist-id]').length);

{
  const page = await openApp();
  if (!(await page.locator('.ff-picker').count())) await page.locator('[data-drop-ribbon]').click();
  await page.waitForSelector('.ff-picker');
  const before = await playlists(page);
  const hovered = await dragTrackTo(page, '.ff-dcard--new', 'picker-card');
  results.push(`picker card: highlighted=${hovered} playlists ${before} -> ${await playlists(page)}`);
  await page.context().close();
}
for (const [name, selector] of [
  ['sidebar fill', '.ff-sidebar__fill'],
  ['sidebar +', '.ff-sidebar-section__add-btn'],
]) {
  const page = await openApp();
  const before = await playlists(page);
  const hovered = await dragTrackTo(page, selector, name.replace(/\W+/g, '-'));
  results.push(`${name}: highlighted=${hovered} playlists ${before} -> ${await playlists(page)}`);
  await page.context().close();
}
console.log(results.join('\n'));
await browser.close();
