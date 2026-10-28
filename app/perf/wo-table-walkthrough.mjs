import { createRequire } from 'node:module';
import { mkdirSync } from 'node:fs';

const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [base = 'http://127.0.0.1:4417/', outDir = './wo-shots'] = process.argv.slice(2);
mkdirSync(outDir, { recursive: true });

const browser = await chromium.launch(process.env.CHROME ? { executablePath: process.env.CHROME } : {});
const results = [];
const note = (line) => results.push(line);

const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
const page = await ctx.newPage();
const dialogs = [];
let dialogAnswer = false;
page.on('dialog', async (d) => {
  dialogs.push(d.message());
  await (dialogAnswer ? d.accept() : d.dismiss());
});
await page.goto(`${base}?mock=200`, { waitUntil: 'commit', timeout: 0 });
await page.waitForSelector('[data-track-row]', { timeout: 60000 });
await page.waitForTimeout(1500);

const shot = (name, clip) => page.screenshot({ path: `${outDir}/${name}.png`, clip });

const headerBox = await page.locator('.ff-colh').boundingBox();
const headerClip = { x: headerBox.x, y: headerBox.y, width: 820, height: headerBox.height };
await shot('header-after', headerClip);
await page.addStyleTag({
  content:
    '.ff-colh__resize-handle::after{top:3px!important;bottom:3px!important;height:auto!important;margin-top:0!important}',
});
await shot('header-before-dividers-full-height', headerClip);
await page.reload({ waitUntil: 'commit' });
await page.waitForSelector('[data-track-row]');
await page.waitForTimeout(1500);

const cells = await page.evaluate(() =>
  [...document.querySelectorAll('.ff-colh__cell')].map((c) => {
    const r = c.getBoundingClientRect();
    return { key: c.getAttribute('data-col-key'), w: Math.round(r.width), h: Math.round(r.height) };
  })
);
note(`header cells: ${cells.map((c) => `${c.key}:${c.w}x${c.h}`).join(' ')}`);

const coverCell = page.locator('.ff-colh__cell[data-col-key="cover"]');
if (await coverCell.count()) {
  const box = await coverCell.boundingBox();
  const before = await page.evaluate(() => [...document.querySelectorAll('.ff-colh__cell')].map((c) => c.getAttribute('data-col-key')));
  const target = await page.locator('.ff-colh__cell[data-col-key="title"]').boundingBox();
  await page.mouse.move(box.x + box.width / 2 - 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2 + 30, box.y + box.height / 2, { steps: 5 });
  await page.mouse.move(target.x + target.width - 10, target.y + target.height / 2, { steps: 10 });
  await page.mouse.up();
  await page.waitForTimeout(300);
  const after = await page.evaluate(() => [...document.querySelectorAll('.ff-colh__cell')].map((c) => c.getAttribute('data-col-key')));
  note(`cover header drag (grab the nameless cell): ${before.join(',')} -> ${after.join(',')}`);
}

const waveCell = page.locator('[data-track-row]').nth(2).locator('[data-wave-seek]');
const wb = await waveCell.boundingBox();
const rowBox = await page.locator('[data-track-row]').nth(2).boundingBox();
await page.mouse.move(wb.x + wb.width * 0.4, wb.y + wb.height / 2);
await page.waitForTimeout(200);
const cursor = await page.locator('[data-track-row]').nth(2).locator('.ff-trow__wave-cursor').boundingBox();
note(`wave cursor top=${cursor.y} h=${cursor.height}; row top=${rowBox.y} h=${rowBox.height}`);
await shot('row-cursor', { x: rowBox.x, y: rowBox.y - 28, width: 700, height: rowBox.height * 3 + 28 });

await page.locator('.ff-colh').click({ button: 'right', position: { x: 400, y: 14 } });
await page.waitForTimeout(200);
await page.locator('.ff-menu__item-label', { hasText: 'Rating' }).first().click();
await page.waitForTimeout(400);
const rateRow = page.locator('[data-track-row]').nth(4);
const stars = rateRow.locator('[data-star]');
note(`stars rendered per row: ${await stars.count()} (unrated row)`);
const rbox = await rateRow.boundingBox();
await shot('stars-unrated', { x: rbox.x, y: rbox.y - 28, width: 1000, height: rbox.height * 2 + 28 });
await stars.nth(3).hover();
await page.waitForTimeout(150);
const hoverOn = await rateRow.locator('.ff-trow__star--on').count();
note(`hover on star 4 previews ${hoverOn} lit stars`);
await shot('stars-hover', { x: rbox.x, y: rbox.y - 28, width: 1000, height: rbox.height * 2 + 28 });
await stars.nth(3).click();
await page.mouse.move(rbox.x + 400, rbox.y + 10);
await page.waitForTimeout(200);
const afterClick = await rateRow.locator('.ff-trow__star--on').count();
note(`after click star 4: ${afterClick} lit`);
await shot('stars-rated', { x: rbox.x, y: rbox.y - 28, width: 1000, height: rbox.height * 2 + 28 });
await stars.nth(3).click();
await page.mouse.move(rbox.x + 400, rbox.y + 10);
await page.waitForTimeout(200);
note(`click again clears: ${await rateRow.locator('.ff-trow__star--on').count()} lit`);

const playlistRows = page.locator('.ff-sidebar__drop-wrap[data-playlist-id]');
const countPlaylists = () => playlistRows.count();
const startCount = await countPlaylists();
await playlistRows.first().click({ button: 'right' });
await page.waitForTimeout(200);
await page.locator('.ff-menu__item-label', { hasText: /^Delete playlist/ }).first().click();
await page.waitForTimeout(400);
note(`menu delete, answer Cancel: dialogs=${JSON.stringify(dialogs)} playlists ${startCount} -> ${await countPlaylists()}`);
dialogs.length = 0;
dialogAnswer = true;
await playlistRows.first().click({ button: 'right' });
await page.waitForTimeout(200);
await page.locator('.ff-menu__item-label', { hasText: /^Delete playlist/ }).first().click();
await page.waitForTimeout(400);
note(`menu delete, answer OK: dialogs=${JSON.stringify(dialogs)} playlists ${await countPlaylists()}`);

await browser.close();
console.log(results.join('\n'));
