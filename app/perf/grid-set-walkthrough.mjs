// Headless check of beat-grid editing in the expanded player (browser-mock mode): after a pause, Set puts the downbeat
// exactly under the playhead, and each ⌥ nudge moves it by 1 ms at the deepest zoom. Needs playwright; run:
// APP_URL=http://127.0.0.1:5200/?mock=200 OUT_DIR=... node perf/grid-set-walkthrough.mjs
import { chromium } from 'playwright';
import { mkdirSync } from 'node:fs';

const out = (process.env.OUT_DIR ?? 'feedback-grid') + '/';
mkdirSync(out, { recursive: true });
const url = process.env.APP_URL ?? 'http://127.0.0.1:5200/?mock=200';
const browser = await chromium.launch({ executablePath: process.env.PW_EXE || undefined });
const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
const p = await ctx.newPage();
let failed = 0;
const ok = (c, msg) => { console.log((c ? 'PASS ' : 'FAIL ') + msg); if (!c) failed++; };
const settle = (ms = 400) => p.waitForTimeout(ms);

const brightColumns = () => p.evaluate(() => {
  const canvas = document.querySelector('.ff-xp .waveform-display__zoom');
  const { width, height, data } = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height);
  const cols = [];
  for (let x = 0; x < width; x++) {
    let bright = 0;
    for (let y = 30; y < height - 30; y += 4) {
      const i = (y * width + x) * 4;
      if (data[i] > 150 && data[i + 1] > 150 && data[i + 2] > 150) bright++;
    }
    if (bright > ((height - 60) / 4) * 0.9) cols.push(x);
  }
  return { centre: width / 2, cols };
});
const gridLine = async () => {
  const { centre, cols } = await brightColumns();
  const off = cols.filter((x) => Math.abs(x - centre) > 2);
  return { centre, line: off.length ? off[0] : centre };
};

await p.goto(url);
await settle(1500);
await p.locator('text=Track 3').first().dblclick();
await settle(900);
await p.evaluate(() => document.activeElement?.blur?.());
await p.keyboard.press('Space');
await settle(1300);
await p.evaluate(() => document.activeElement?.blur?.());
await p.keyboard.press('Tab');
await settle(600);
if ((await p.locator('.ff-xp__play').getAttribute('aria-label')) === 'Pause') await p.locator('.ff-xp__play').click();
await settle(600);
for (let i = 0; i < 30; i++) await p.locator('.ff-xp__zoom-btn[aria-label="Zoom in"]').click();
await settle(500);

await p.locator('.ff-xp__grid-btn[aria-label="Set downbeat to playhead"]').click();
await settle();
const set = await gridLine();
ok(set.line === set.centre, `Set after a pause puts the downbeat under the playhead (line ${set.line}, centre ${set.centre})`);

for (let i = 0; i < 3; i++) await p.locator('.ff-xp__grid-btn[aria-label="Nudge grid right"]').click({ modifiers: ['Alt'] });
await settle();
const three = await gridLine();
for (let i = 0; i < 3; i++) await p.locator('.ff-xp__grid-btn[aria-label="Nudge grid right"]').click({ modifiers: ['Alt'] });
await settle();
const six = await gridLine();
const perMs = (three.line - set.centre) / 3;
ok(perMs > 1, `three ⌥ nudges move the downbeat right by whole milliseconds (${perMs.toFixed(2)} device px per ms)`);
ok(Math.abs(six.line - set.centre - 6 * perMs) <= 1, `six ⌥ nudges land at 6 ms (${six.line - set.centre} px)`);

const box = await p.locator('.ff-xp .waveform-display__zoom').boundingBox();
await p.mouse.click(box.x + box.width * 0.5 + 37, box.y + box.height / 2);
await settle();
await p.locator('.ff-xp__grid-btn[aria-label="Set downbeat to playhead"]').click();
await settle();
const clicked = await gridLine();
ok(clicked.line === clicked.centre, 'Set after a click on the zoom view puts the downbeat under the playhead');
await p.screenshot({ path: out + 'grid-set.png' });
await browser.close();
process.exit(failed ? 1 : 0);
