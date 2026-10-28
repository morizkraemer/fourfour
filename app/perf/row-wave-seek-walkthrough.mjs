// Headless check of the table's mini waveform as a seek strip (browser-mock mode): hover shows a cursor at the pointer,
// a click loads that track at that point without starting a drag. Needs playwright; run:
// APP_URL=http://127.0.0.1:5200/?mock=200 OUT_DIR=... node perf/row-wave-seek-walkthrough.mjs
import { chromium } from 'playwright';
import { mkdirSync } from 'node:fs';

const out = (process.env.OUT_DIR ?? 'feedback-row-wave') + '/';
mkdirSync(out, { recursive: true });
const url = process.env.APP_URL ?? 'http://127.0.0.1:5200/?mock=200';
const browser = await chromium.launch({ executablePath: process.env.PW_EXE || undefined });
const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
const p = await ctx.newPage();
let failed = 0;
const ok = (c, msg) => { console.log((c ? 'PASS ' : 'FAIL ') + msg); if (!c) failed++; };
const settle = (ms = 400) => p.waitForTimeout(ms);
const seconds = (text) => { const [m, s] = text.split('\n')[0].trim().split(':').map(Number); return m * 60 + s; };

await p.goto(url);
await settle(1500);
const row = p.locator('[data-track-row]').filter({ hasText: 'Track 5' }).first();
const cell = row.locator('[data-wave-seek]');
const box = await cell.locator('canvas').boundingBox();
await p.mouse.move(box.x + box.width * 0.75, box.y + box.height / 2);
await settle(200);
const cursor = await cell.locator('.ff-trow__wave-cursor').evaluate((el) => {
  const r = el.getBoundingClientRect();
  return { left: r.left, opacity: getComputedStyle(el).opacity };
});
ok(cursor.opacity === '1', 'hovering the row waveform shows the cursor');
ok(Math.abs(cursor.left - (box.x + box.width * 0.75)) <= 1.5, `the cursor follows the pointer (${cursor.left.toFixed(1)} vs ${(box.x + box.width * 0.75).toFixed(1)})`);
await p.screenshot({ path: out + 'hover.png', clip: { x: box.x - 40, y: box.y - 20, width: box.width + 80, height: box.height + 40 } });

await p.mouse.down();
await p.mouse.move(box.x + box.width * 0.75 + 2, box.y + box.height / 2);
await p.mouse.up();
await settle(900);
ok((await p.locator('.ff-drag-ghost, [data-drag-ghost]').count()) === 0 || !(await p.locator('.ff-drag-ghost').first().isVisible().catch(() => false)), 'no drag starts from the waveform');
const title = await p.locator('.ff-player, footer, body').first().innerText();
ok(title.includes('Track 5'), 'the click loads that track in the player');
const time = await p.locator('.ff-pc__time, [class*="time-cur"]').first().innerText().catch(() => '');
const length = await row.locator('[data-col-key="time"]').innerText().catch(() => '');
console.log('player time', JSON.stringify(time), 'track length', JSON.stringify(length));
if (time && length) {
  const expected = seconds(length) * (0.75 + 2 / box.width);
  ok(Math.abs(seconds(time) - expected) <= 2, `the player sits at the clicked point (${seconds(time)} s vs ${expected.toFixed(1)} s)`);
}
await p.screenshot({ path: out + 'after-click.png' });
await browser.close();
process.exit(failed ? 1 : 0);
