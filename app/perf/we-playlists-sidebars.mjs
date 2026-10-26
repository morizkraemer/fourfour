import { createRequire } from 'node:module';
import { mkdirSync } from 'node:fs';

const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [base = 'http://127.0.0.1:4411/', outDir = './we-shots'] = process.argv.slice(2);
mkdirSync(outDir, { recursive: true });

const browser = await chromium.launch();
const errors = [];
const steps = [];

async function newPage(width, height = 900) {
  const ctx = await browser.newContext({ viewport: { width, height } });
  const page = await ctx.newPage();
  page.on('console', (m) => {
    if (m.type() === 'error') errors.push(`console: ${m.text()}`);
  });
  page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));
  await page.goto(`${base}?mock=200`, { waitUntil: 'commit', timeout: 0 });
  await page.waitForSelector('[data-track-row]', { timeout: 60000 });
  await page.waitForTimeout(1200);
  return page;
}

async function step(name, fn) {
  try {
    const note = await fn();
    steps.push(`ok   ${name}${note ? ` (${note})` : ''}`);
  } catch (e) {
    steps.push(`FAIL ${name}: ${e.message.split('\n')[0]}`);
  }
}

const shot = (page, name) => page.screenshot({ path: `${outDir}/${name}.png` });
const playlistCount = (page) => page.evaluate(() => document.querySelectorAll('.ff-sidebar__drop-wrap').length);

async function dragRowsToPlus(page, rowIdxs, name) {
  const rows = page.locator('[data-track-row]');
  await rows.nth(rowIdxs[0]).click();
  if (rowIdxs.length > 1) await rows.nth(rowIdxs[rowIdxs.length - 1]).click({ modifiers: ['Shift'] });
  const src = await rows.nth(rowIdxs[0]).boundingBox();
  const plus = await page.locator('[data-new-playlist-drop]').boundingBox();
  await page.mouse.move(src.x + 200, src.y + src.height / 2);
  await page.mouse.down();
  await page.mouse.move(src.x + 160, src.y + 40, { steps: 5 });
  await page.mouse.move(plus.x + plus.width / 2, plus.y + plus.height / 2, { steps: 15 });
  await page.waitForTimeout(250);
  const lit = await page.locator('.ff-sidebar-section__add-btn--drop').count();
  await shot(page, `${name}-hover`);
  await page.mouse.up();
  await page.waitForTimeout(600);
  await shot(page, `${name}-after`);
  return lit;
}

let page = await newPage(1440);
await step('drag 1 track onto +', async () => {
  const before = await playlistCount(page);
  const lit = await dragRowsToPlus(page, [2], 'f7-one-track');
  if (!lit) throw new Error('+ not highlighted');
  const after = await playlistCount(page);
  if (after !== before + 1) throw new Error(`playlists ${before} -> ${after}`);
  const header = await page.locator('.ff-organize__header-rename').count();
  if (!header) throw new Error('no inline rename in panel');
  const rows = await page.locator('.ff-organize [data-track-row]').count();
  if (rows !== 1) throw new Error(`panel rows ${rows}`);
});
await page.keyboard.press('Enter');
await page.waitForTimeout(300);
await step('drag 3 tracks onto +', async () => {
  const lit = await dragRowsToPlus(page, [4, 6], 'f7-three-tracks');
  if (!lit) throw new Error('+ not highlighted');
  const rows = await page.locator('.ff-organize [data-track-row]').count();
  if (rows !== 3) throw new Error(`panel rows ${rows}`);
  const title = await page.locator('.ff-organize__header-rename').count();
  if (!title) throw new Error('no inline rename');
});
await page.keyboard.press('Enter');
await page.close();

page = await newPage(1440);
await step('+ at 1440 docks in panel, left view unchanged', async () => {
  const active = await page.locator('.ff-sbrow--active .ff-sbrow__label').first().textContent();
  await page.locator('.ff-sidebar-section__add-btn').click();
  await page.waitForTimeout(500);
  const expanded = await page.locator('.ff-organize--expanded').count();
  const rename = await page.locator('.ff-organize__header-rename').count();
  const activeAfter = await page.locator('.ff-sbrow--active .ff-sbrow__label').first().textContent();
  await shot(page, 'f8-plus-1440');
  if (!expanded || !rename) throw new Error(`expanded=${expanded} rename=${rename}`);
  if (active !== activeAfter) throw new Error(`left view changed ${active} -> ${activeAfter}`);
});
await page.close();

page = await newPage(1100);
await step('+ at 1100 docks in panel, left view unchanged', async () => {
  const active = await page.locator('.ff-sbrow--active .ff-sbrow__label').first().textContent();
  await page.locator('.ff-sidebar-section__add-btn').click();
  await page.waitForTimeout(500);
  const expanded = await page.locator('.ff-organize--expanded').count();
  const activeAfter = await page.locator('.ff-sbrow--active .ff-sbrow__label').first().textContent();
  await shot(page, 'f8-plus-1100');
  if (!expanded) throw new Error('panel not expanded');
  if (active !== activeAfter) throw new Error(`left view changed ${active} -> ${activeAfter}`);
});
await page.close();

page = await newPage(1000);
await step('+ at 1000 falls back to opening the playlist on the left', async () => {
  await page.locator('.ff-sidebar-section__add-btn').click();
  await page.waitForTimeout(500);
  const active = await page.locator('.ff-sbrow--active .ff-sbrow__label').first().textContent();
  await shot(page, 'f8-plus-1000');
  if (!/^Playlist \d+/.test(active ?? '')) throw new Error(`active row ${active}`);
});
await page.close();

page = await newPage(1440);
await step('resize sidebar and inspector, persist across reload', async () => {
  await page.locator('[data-track-row]').nth(3).click();
  await page.waitForTimeout(300);
  const sb = await page.locator('.ff-sidebar').boundingBox();
  await shot(page, 'f9-before');
  const grip = await page.locator('[aria-label="Resize sidebar"]').boundingBox();
  await page.mouse.move(grip.x + grip.width / 2, grip.y + 300);
  await page.mouse.down();
  await page.mouse.move(grip.x + 100, grip.y + 300, { steps: 8 });
  await page.waitForTimeout(100);
  await shot(page, 'f9-dragging');
  await page.mouse.up();
  const det = await page.locator('.ff-browse__slot--detail').boundingBox();
  const g2 = await page.locator('[aria-label="Resize inspector"]').boundingBox();
  await page.mouse.move(g2.x + g2.width / 2, g2.y + 300);
  await page.mouse.down();
  await page.mouse.move(g2.x - 120, g2.y + 300, { steps: 8 });
  await page.mouse.up();
  const sb2 = await page.locator('.ff-sidebar').boundingBox();
  const det2 = await page.locator('.ff-browse__slot--detail').boundingBox();
  const table = await page.locator('.ff-browse__slot--table').boundingBox();
  await shot(page, 'f9-resized');
  if (Math.round(sb2.width) !== Math.round(sb.width + 100)) throw new Error(`sidebar ${sb.width} -> ${sb2.width}`);
  if (Math.round(det2.width) !== Math.round(det.width + 120)) throw new Error(`inspector ${det.width} -> ${det2.width}`);
  if (Math.abs(sb2.x + sb2.width - table.x) > 1 || Math.abs(table.x + table.width - det2.x) > 1) throw new Error('table does not fill the gap');
  await page.reload({ waitUntil: 'commit' });
  await page.waitForSelector('[data-track-row]', { timeout: 60000 });
  await page.waitForTimeout(1000);
  await page.locator('[data-track-row]').nth(3).click();
  await page.waitForTimeout(300);
  const sb3 = await page.locator('.ff-sidebar').boundingBox();
  const det3 = await page.locator('.ff-browse__slot--detail').boundingBox();
  await shot(page, 'f9-after-reload');
  if (Math.round(sb3.width) !== Math.round(sb2.width) || Math.round(det3.width) !== Math.round(det2.width)) {
    throw new Error(`not persisted: ${sb3.width}/${det3.width}`);
  }
});
await step('clamp to min/max and double-click reset', async () => {
  const grip = await page.locator('[aria-label="Resize sidebar"]').boundingBox();
  await page.mouse.move(grip.x + grip.width / 2, grip.y + 300);
  await page.mouse.down();
  await page.mouse.move(grip.x + 900, grip.y + 300, { steps: 6 });
  await page.mouse.up();
  const wide = (await page.locator('.ff-sidebar').boundingBox()).width;
  if (Math.round(wide) !== 420) throw new Error(`max not clamped: ${wide}`);
  await page.locator('[aria-label="Resize sidebar"]').dblclick();
  await page.waitForTimeout(200);
  const reset = (await page.locator('.ff-sidebar').boundingBox()).width;
  await page.locator('[aria-label="Resize inspector"]').dblclick();
  await page.waitForTimeout(200);
  const detReset = (await page.locator('.ff-browse__slot--detail').boundingBox()).width;
  if (Math.round(reset) !== 240 || Math.round(detReset) !== 278) throw new Error(`reset ${reset}/${detReset}`);
});
await page.close();

page = await newPage(1440);
await step('favourites menu: move to new playlist copies and keeps favourites', async () => {
  await page.locator('[data-track-row]').nth(1).click();
  await page.keyboard.press('1');
  await page.locator('[data-track-row]').nth(2).click();
  await page.keyboard.press('1');
  await page.waitForTimeout(400);
  const header = page.locator('.ff-sidebar-section__label', { hasText: 'FAVORITES' });
  await header.click({ button: 'right' });
  await page.waitForTimeout(300);
  await shot(page, 'f11-menu');
  const labels = await page.locator('[role="menuitem"], .ff-menu__item').allTextContents();
  if (!labels.some((l) => /Move favourite tracks to new playlist/.test(l))) throw new Error(`menu: ${labels}`);
  if (!labels.some((l) => /Clear favourites/.test(l))) throw new Error(`no clear item: ${labels}`);
  await page.getByText('Move favourite tracks to new playlist').click();
  await page.waitForTimeout(600);
  const rows = await page.locator('.ff-organize [data-track-row]').count();
  const favCount = await page.locator('.ff-sbrow', { hasText: 'Favorites 1' }).locator('.ff-sbrow__count').textContent();
  await shot(page, 'f11-after');
  if (rows !== 2) throw new Error(`new playlist rows ${rows}`);
  if (favCount.trim() !== '2') throw new Error(`favourites changed: ${favCount}`);
});
await page.close();

await browser.close();
console.log(steps.join('\n'));
console.log(errors.length ? `ERRORS:\n${errors.join('\n')}` : 'no console errors');
process.exit(steps.some((s) => s.startsWith('FAIL')) || errors.length ? 1 : 0);
