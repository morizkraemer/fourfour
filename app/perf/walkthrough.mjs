import { createRequire } from 'node:module';

const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [base = 'http://127.0.0.1:4311/', count = '2000'] = process.argv.slice(2);
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const errors = [];
page.on('console', (m) => {
  if (m.type() === 'error') errors.push(`console: ${m.text()}`);
});
page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));

const steps = [];
async function step(name, fn) {
  try {
    await fn();
    steps.push(`ok   ${name}`);
  } catch (e) {
    steps.push(`FAIL ${name}: ${e.message.split('\n')[0]}`);
  }
  await page.waitForTimeout(400);
}

const rowCount = () => page.locator('[data-track-row]').count();

await page.goto(`${base}?mock=${count}`, { waitUntil: 'commit', timeout: 0 });
await page.waitForSelector('[data-track-row]', { timeout: 0 });
await page.waitForTimeout(1500);

await step('rows virtualised', async () => {
  const n = await rowCount();
  if (n === 0 || (Number(count) > 200 && n > 120)) throw new Error(`rows in DOM: ${n}`);
});
await step('select row', async () => {
  await page.locator('[data-track-row]').nth(4).click();
  if (!(await page.locator('.ff-trow--selected').count())) throw new Error('no selected row');
});
await step('shift-click range', async () => {
  await page.locator('[data-track-row]').nth(9).click({ modifiers: ['Shift'] });
  const n = await page.locator('.ff-trow--selected').count();
  if (n !== 6) throw new Error(`selected ${n}, expected 6`);
});
await step('scroll to end and select last row', async () => {
  await page.evaluate(() => {
    const el = document.querySelector('.ff-table__scroll');
    el.scrollTop = el.scrollHeight;
  });
  await page.waitForTimeout(500);
  const last = page.locator('[data-track-row]').last();
  const idx = Number(await last.getAttribute('data-row-index'));
  if (idx !== Number(count) - 1) throw new Error(`last rendered index ${idx}`);
  await last.click();
});
await step('sort by title', async () => {
  await page.locator('.ff-colh__cell[data-col-key="title"]').first().click();
});
await step('column context menu', async () => {
  await page.locator('.ff-colh').first().click({ button: 'right' });
  await page.keyboard.press('Escape');
});
await step('search filters rows', async () => {
  await page.locator('input').first().click();
  await page.keyboard.type('track 12');
  await page.waitForTimeout(500);
  const first = await page.locator('[data-track-row]').first().innerText();
  if (!first.includes('12')) throw new Error(`first filtered row: ${first}`);
  await page.keyboard.press('Escape');
});
await step('marquee select', async () => {
  const box = await page.locator('.ff-table__scroll').boundingBox();
  await page.evaluate(() => (document.querySelector('.ff-table__scroll').scrollTop = 0));
  await page.waitForTimeout(300);
  await page.mouse.move(box.x + box.width - 6, box.y + 80);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width - 6, box.y + 200, { steps: 6 });
  await page.mouse.up();
});
await step('drag row onto playlist', async () => {
  const row = page.locator('[data-track-row]').nth(2);
  const box = await row.boundingBox();
  await page.mouse.move(box.x + 40, box.y + 10);
  await page.mouse.down();
  await page.mouse.move(box.x + 20, box.y + 40, { steps: 5 });
  await page.mouse.move(60, 400, { steps: 8 });
  await page.mouse.up();
});
await step('organize panel', async () => {
  await page.getByLabel('Open organize panel').click();
  await page.waitForTimeout(500);
});
await step('settings', async () => {
  await page.getByLabel('Settings').first().click();
});

console.log(steps.join('\n'));
console.log('console errors:', errors.length ? errors : 'none');
await browser.close();
