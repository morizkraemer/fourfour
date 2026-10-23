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
  const idx = await page.evaluate(() => Math.max(...[...document.querySelectorAll('[data-track-row]')].map((r) => Number(r.dataset.rowIndex))));
  if (idx !== Number(count) - 1) throw new Error(`last rendered index ${idx}`);
  await page.locator(`[data-track-row][data-row-index="${idx}"]`).click();
  const selected = await page.evaluate(() => [...document.querySelectorAll('.ff-trow--selected')].map((r) => r.parentElement.dataset.rowIndex));
  if (selected.length !== 1 || Number(selected[0]) !== idx) throw new Error(`selected ${selected}`);
});
await step('rows match their index after scrolling', async () => {
  for (const top of [0, 5000, 123456, 99999, 7777, 1e9]) {
    await page.evaluate((t) => (document.querySelector('.ff-table__scroll').scrollTop = t), top);
    await page.waitForTimeout(250);
    const bad = await page.evaluate(() => {
      const scroll = document.querySelector('.ff-table__scroll');
      const header = scroll.querySelector('.ff-colh').offsetHeight;
      const out = [];
      const rows = [...document.querySelectorAll('[data-track-row]')];
      for (const r of rows) {
        const i = Number(r.dataset.rowIndex);
        const title = r.querySelector('[data-col-key="title"]')?.textContent.trim();
        if (title !== `Track ${i}` && !/^Track \d+$/.test(title ?? '')) out.push(`row ${i}: ${title}`);
        const rect = r.getBoundingClientRect();
        const expectTop = scroll.getBoundingClientRect().top + header + i * 28 - scroll.scrollTop;
        if (Math.abs(rect.top - expectTop) > 0.5) out.push(`row ${i} top ${rect.top} != ${expectTop}`);
      }
      const idxs = rows.map((r) => Number(r.dataset.rowIndex)).sort((a, b) => a - b);
      for (let k = 1; k < idxs.length; k++) if (idxs[k] !== idxs[k - 1] + 1) out.push(`gap at ${idxs[k]}`);
      return out;
    });
    if (bad.length) throw new Error(`scrollTop ${top}: ${bad.slice(0, 3).join('; ')}`);
  }
});
await step('column resize', async () => {
  await page.evaluate(() => (document.querySelector('.ff-table__scroll').scrollTop = 0));
  const handle = page.locator('.ff-colh__cell[data-col-key="artist"] .ff-colh__resize-handle').first();
  const widthOf = () => page.locator('[data-track-row] [data-col-key="artist"]').first().evaluate((e) => e.getBoundingClientRect().width);
  const before = await widthOf();
  await handle.evaluate((el) => {
    const x = el.getBoundingClientRect().left + 4;
    const fire = (target, type, clientX) =>
      target.dispatchEvent(new PointerEvent(type, { bubbles: true, cancelable: true, button: 0, clientX, clientY: 50, pointerId: 1 }));
    fire(el, 'pointerdown', x);
    for (let i = 1; i <= 6; i++) fire(window, 'pointermove', x - (40 * i) / 6);
    fire(window, 'pointerup', x - 40);
  });
  await page.waitForTimeout(200);
  const after = await widthOf();
  if (Math.abs(before - after - 40) > 3) throw new Error(`artist width ${before} -> ${after}`);
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
await step('drop index in playlist', async () => {
  await page.getByText('Peak Time', { exact: true }).first().click();
  await page.waitForTimeout(400);
  const row = page.locator('[data-track-row]').nth(0);
  const box = await row.boundingBox();
  await page.mouse.move(box.x + 40, box.y + 10);
  await page.mouse.down();
  await page.mouse.move(box.x + 40, box.y + 20, { steps: 3 });
  await page.mouse.move(box.x + 40, box.y + 50, { steps: 5 });
  const top = await page.locator('.ff-table__drop-line').first().evaluate((e) => e.style.top);
  await page.mouse.up();
  if (top !== '56px') throw new Error(`drop line top ${top}`);
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
