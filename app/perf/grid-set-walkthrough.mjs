// Headless check of beat-grid editing in the expanded player (browser-mock mode), at the deepest zoom: Set puts the
// downbeat exactly under the playhead after a click between two milliseconds, a nudge moves it one CSS pixel, ⌥ one
// sample, and after saving and loading the track again the grid is where it was set, to the device pixel.
// PW_DIR=<dir with playwright> APP_URL=http://127.0.0.1:5200/?mock=200 OUT_DIR=... node perf/grid-set-walkthrough.mjs
import { createRequire } from 'node:module';
import { mkdirSync } from 'node:fs';

const require = createRequire(`${process.env.PW_DIR ?? process.cwd()}/`);
const { chromium } = require('playwright');
const out = (process.env.OUT_DIR ?? 'feedback-grid') + '/';
mkdirSync(out, { recursive: true });
const url = process.env.APP_URL ?? 'http://127.0.0.1:5200/?mock=200';
const browser = await chromium.launch({ executablePath: process.env.PW_EXE || undefined });
const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
const p = await ctx.newPage();
const errors = [];
p.on('pageerror', (e) => errors.push(String(e)));
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
  return { centre: Math.floor(width / 2), cols };
});
const gridLine = async () => {
  const { centre, cols } = await brightColumns();
  const off = cols.filter((x) => Math.abs(x - centre) > 2);
  return { centre, line: off.length ? off[0] : centre };
};
const playerModule = `(performance.getEntriesByType('resource').map((e) => e.name).find((n) => n.includes('/src/stores/player.svelte.ts')) ?? '/src/stores/player.svelte.ts')`;
// The store module as the app loaded it (after a hot update its URL carries a timestamp), so this reads the live state.
const store = () => p.evaluate(async (from) => {
  const { player, currentPositionMs } = await import(eval(from));
  return { beats: player.track.beats.map((b) => b.time_ms), positionMs: currentPositionMs(), id: player.track.id };
}, playerModule);
const zoomAllTheWay = async () => {
  for (let i = 0; i < 40; i++) await p.locator('.ff-xp__zoom-btn[aria-label="Zoom in"]').click();
  await settle(500);
};
const msPerDevicePx = () => p.evaluate(() => {
  const canvas = document.querySelector('.ff-xp .waveform-display__zoom');
  return canvas.width;
});

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
await zoomAllTheWay();
const widthPx = await msPerDevicePx();

const box = await p.locator('.ff-xp .waveform-display__zoom').boundingBox();
await p.mouse.click(box.x + box.width * 0.5 + 37.3, box.y + box.height / 2);
await settle(800);
const clickedAt = (await store()).positionMs;
ok(Math.abs(clickedAt - Math.round(clickedAt)) > 1e-6, `the click rests between two milliseconds (${clickedAt.toFixed(4)} ms), not on the reported whole ms`);
await p.locator('.ff-xp__grid-btn[aria-label="Set downbeat to playhead"]').click();
await settle();
const set = await gridLine();
ok(set.line === set.centre, `Set puts the downbeat under the playhead (line ${set.line}, centre ${set.centre})`);
const afterSet = await store();
ok(afterSet.beats.some((t) => Math.abs(t - clickedAt) < 1e-6), 'a downbeat lies exactly at the playhead time');

for (let i = 0; i < 3; i++) await p.locator('.ff-xp__grid-btn[aria-label="Nudge grid right"]').click();
await settle();
const nudged = await gridLine();
ok(Math.abs(nudged.line - set.centre - 6) <= 1, `three nudges move the grid three CSS pixels (${nudged.line - set.centre} device px)`);
for (let i = 0; i < 3; i++) await p.locator('.ff-xp__grid-btn[aria-label="Nudge grid left"]').click();
await settle();
const back = await gridLine();
ok(back.line === back.centre, 'nudging back returns it under the playhead');
const before = (await store()).beats;
await p.locator('.ff-xp__grid-btn[aria-label="Nudge grid right"]').click({ modifiers: ['Alt'] });
await settle();
const fine = (await store()).beats;
ok(Math.abs(fine[0] - before[0] - 1000 / 44100) < 1e-6, `⌥ nudges one sample (${(fine[0] - before[0]).toFixed(5)} ms)`);
await p.locator('.ff-xp__grid-btn[aria-label="Nudge grid left"]').click({ modifiers: ['Alt'] });
await settle(1200);

const saved = (await store()).beats;
await p.locator('text=Track 4').first().dblclick();
await settle(1200);
await p.locator('text=Track 3').first().dblclick();
await settle(1500);
const reloaded = await store();
ok(reloaded.beats.length === saved.length && reloaded.beats.every((t, i) => Math.abs(t - saved[i]) < 1e-6),
  `after saving and loading the track again the beats are exact (first ${reloaded.beats[0]?.toFixed(4)} ms, was ${saved[0]?.toFixed(4)})`);
await p.evaluate(async ([from, ms]) => {
  const { seekToMs } = await import(eval(from));
  await seekToMs(ms);
}, [playerModule, clickedAt]);
await settle(800);
await zoomAllTheWay();
const again = await gridLine();
ok(again.line === again.centre, `reloaded, the downbeat draws under the playhead at the deepest zoom (line ${again.line}, centre ${again.centre}, ${widthPx} px wide)`);
await p.screenshot({ path: out + 'grid-set.png' });
ok(errors.length === 0, `no page errors ${errors.join(' | ')}`);
await browser.close();
process.exit(failed ? 1 : 0);
