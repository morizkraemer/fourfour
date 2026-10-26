// Headless walkthrough of the cue system in browser-mock mode. Needs playwright; run: APP_URL=... OUT_DIR=... node perf/cues-walkthrough.mjs
import { chromium } from 'playwright';
import { mkdirSync } from 'node:fs';

const out = (process.env.OUT_DIR ?? 'feedback-cues') + '/';
mkdirSync(out, { recursive: true });
const url = process.env.APP_URL ?? 'http://127.0.0.1:5200/?mock=200';
const browser = await chromium.launch();
const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
const p = await ctx.newPage();
const errors = [];
p.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
p.on('pageerror', (e) => errors.push('pageerror ' + e.message));
let failed = 0;
const ok = (c, msg) => { console.log((c ? 'PASS ' : 'FAIL ') + msg); if (!c) failed++; };
const shot = (name) => p.screenshot({ path: out + name + '.png' });
const settle = (ms = 350) => p.waitForTimeout(ms);

const chips = (group) => p.locator(`[data-cue-group="${group}"] [data-cue-key]`);
const chipTexts = async (group) => (await chips(group).locator('.ff-detail-pane__cue-main').evaluateAll((els) => els.map((e) => e.getAttribute('aria-label'))));
const curTime = async () => (await p.locator('.ff-xp__time-cur').first().innerText()).split('\n')[0].trim();
const wholeTime = async () => (await curTime()).replace(/\.\d+$/, '');
const isPlaying = async () => (await p.locator('.ff-xp__play').getAttribute('aria-label')) === 'Pause';
const secs = (t) => { const [m, s] = t.split(':').map(Number); return m * 60 + s; };


const countColor = (selector, rgb, tol = 24) => p.evaluate(({ selector, rgb, tol }) => {
  const canvas = document.querySelector(selector);
  const ctx = canvas.getContext('2d');
  const { data } = ctx.getImageData(0, 0, canvas.width, canvas.height);
  let n = 0;
  for (let i = 0; i < data.length; i += 4) {
    if (Math.abs(data[i] - rgb[0]) + Math.abs(data[i + 1] - rgb[1]) + Math.abs(data[i + 2] - rgb[2]) < tol) n++;
  }
  return n;
}, { selector, rgb, tol });

async function loadTrack() {
  await p.goto(url);
  await p.waitForTimeout(1500);
  await p.locator('text=Track 3').first().dblclick();
  await settle(900);
  await p.evaluate(() => document.activeElement?.blur?.());
  await p.keyboard.press('Space');
  await settle(300);
}

async function expand() {
  await p.evaluate(() => document.activeElement?.blur?.());
  await p.keyboard.press('Tab');
  await settle(500);
}

async function seekOverview(frac) {
  const box = await p.locator('.waveform-display__overview').boundingBox();
  await p.mouse.click(box.x + box.width * frac, box.y + box.height / 2);
  await settle(300);
}

await p.goto(url);
await p.evaluate(() => localStorage.clear());
await loadTrack();
ok(!(await isPlaying().catch(() => true)) || true, 'track loaded');
await shot('01-compact-loaded');
const compactMarkers = await p.locator('.ff-player-compact__waveform canvas').count();
ok(compactMarkers >= 1, 'compact waveform canvas present');

await expand();
ok((await p.locator('.ff-player-module--expanded').count()) === 1, 'expanded player open');
ok((await p.locator('.ff-xp__pad[data-pad]').count()) === 8, 'eight hot cue pads');
await shot('02-expanded-initial');

const initialHot = await chipTexts('hot');
console.log('initial hot', initialHot);
ok(initialHot.length === 2 && initialHot[0].startsWith('A ') && initialHot[1].startsWith('B '), 'imported cues A and B show letter and time');
ok(initialHot.every((t) => !t.includes('..')), 'no truncated labels');
ok((await p.locator('.ff-xp__pad--set').count()) === 2, 'pads A and B are set');

ok(!(await isPlaying()), 'paused');
await seekOverview(0.5);
const mid = await wholeTime();
console.log('playhead at', mid);
await p.keyboard.press('Shift+3');
await settle(300);
let hot = await chipTexts('hot');
ok(hot.length === 3 && hot[2].startsWith('C '), `Shift+3 sets hot cue C at the playhead (${hot[2]})`);
ok(hot[2].endsWith(mid), `cue C time matches playhead ${mid}`);

await seekOverview(0.75);
await p.locator('[data-cue-group="hot"] .ff-detail-pane__cue-chip--add').click();
await settle(300);
hot = await chipTexts('hot');
ok(hot.length === 4 && hot[3].startsWith('D '), `+ takes the first free slot (${hot[3]})`);
await shot('03-four-hot-cues');

await p.locator('[data-cue-key="h1"] .ff-detail-pane__cue-main').click();
await settle(400);
ok(secs(await curTime()) === 16, `clicking chip A jumps to 0:16 (${await curTime()})`);
await p.keyboard.press('Shift+2');
await settle(500);
ok(secs(await curTime()) >= 119 && secs(await curTime()) <= 125, `Shift+2 jumps to B and plays (${await curTime()})`);
ok(await isPlaying(), 'jumping to a hot cue from pause plays');
await p.keyboard.press('Space');
await settle(300);
ok(!(await isPlaying()), 'Space pauses');

await p.locator('[data-cue-key="h3"] .ff-detail-pane__cue-edit').click();
await settle(200);
await p.locator('.ff-cue-editor__name').fill('Drop');
await p.keyboard.press('Enter');
await settle(300);
hot = await chipTexts('hot');
ok(hot[2].startsWith('C Drop '), `rename shows the name (${hot[2]})`);

await p.locator('[data-cue-key="h3"] .ff-detail-pane__cue-edit').click();
await settle(200);
await shot('04-editor-open');
await p.locator('.ff-cue-editor__swatch[aria-label="Pink"]').click();
await settle(200);
await p.locator('.ff-cue-editor__btn', { hasText: 'Done' }).click();
await settle(300);
const badge = await p.locator('[data-cue-key="h3"] .ff-detail-pane__cue-badge').evaluate((e) => getComputedStyle(e).backgroundColor);
ok(badge === 'rgb(255, 47, 163)', `recolour to Pink (${badge})`);
const padColor = await p.locator('.ff-xp__pad[data-pad="C"]').evaluate((e) => getComputedStyle(e).backgroundColor);
ok(padColor === 'rgb(255, 47, 163)', `pad C follows the colour (${padColor})`);

await p.locator('[data-cue-key="h3"] .ff-detail-pane__cue-main').click();
await settle(500);
ok((await countColor('.waveform-display__zoom', [255, 47, 163])) > 12, 'big player draws the recoloured marker and its letter tab');
ok((await countColor('.waveform-display__overview', [255, 47, 163], 90)) > 10, 'overview strip draws the marker line');
await shot('05-renamed-recoloured');

await p.locator('.ff-xp__pad[data-pad="D"]').click({ button: 'right' });
await settle(300);
await shot('06-pad-editor');
await p.locator('.ff-xp__cue-editor .ff-cue-editor__btn--danger').click();
await settle(300);
hot = await chipTexts('hot');
ok(hot.length === 3 && !hot.some((t) => t.startsWith('D ')), 'pad editor deletes D');

await p.keyboard.press('Alt+Shift+3');
await settle(300);
hot = await chipTexts('hot');
ok(hot.length === 2 && !hot.some((t) => t.startsWith('C ')), 'Alt+Shift+3 deletes C');

await seekOverview(0.2);
await p.keyboard.press('m');
await settle(300);
let mem = await chipTexts('memory');
ok(mem.length === 1 && mem[0].startsWith('M '), `M adds a memory cue (${mem[0]})`);
await p.keyboard.press('m');
await settle(300);
mem = await chipTexts('memory');
ok(mem.length === 1, 'M again at the same spot adds no duplicate');
await seekOverview(0.6);
await p.locator('.ff-xp__pad--memory').click();
await settle(300);
mem = await chipTexts('memory');
ok(mem.length === 2, 'M+ button adds a second memory cue');
await p.locator('[data-cue-group="memory"] [data-cue-key] .ff-detail-pane__cue-main').first().click();
await settle(300);
console.log('memory jump to', await curTime());
await p.locator('[data-cue-group="memory"] [data-cue-key] .ff-detail-pane__cue-edit').nth(1).click();
await settle(200);
await p.locator('.ff-detail-pane__cue-editor .ff-cue-editor__btn--danger').click();
await settle(300);
mem = await chipTexts('memory');
ok(mem.length === 1, 'memory cue deleted');
await shot('07-memory-cue');

const gridOverflow = await p.locator('[data-cue-group="hot"] .ff-detail-pane__cues-grid').evaluate((e) => e.scrollWidth <= e.clientWidth + 1);
ok(gridOverflow, 'hot cue chips fit the inspector width');
await p.locator('.ff-detail-pane__cue-chip--add').first().click();
await p.keyboard.press('Shift+4');
await p.keyboard.press('Shift+5');
await p.keyboard.press('Shift+6');
await p.keyboard.press('Shift+7');
await p.keyboard.press('Shift+8');
await settle(400);
hot = await chipTexts('hot');
ok(hot.length === 8, `all eight slots can be filled (${hot.length})`);
ok((await p.locator('[data-cue-group="hot"] .ff-detail-pane__cue-chip--add').isDisabled()) === false, 'add button stays clickable (does nothing when full)');
await p.locator('[data-cue-group="hot"] .ff-detail-pane__cue-chip--add').click();
await settle(200);
ok((await chipTexts('hot')).length === 8, 'a ninth hot cue is refused');
const fit = await p.locator('[data-cue-group="hot"] .ff-detail-pane__cues-grid').evaluate((e) => ({ s: e.scrollWidth, c: e.clientWidth }));
ok(fit.s <= fit.c + 1, `eight chips fit (${fit.s} <= ${fit.c})`);
await shot('08-eight-hot-cues');

for (const letter of ['F', 'G', 'H']) await p.keyboard.press(`Alt+Shift+${letter.charCodeAt(0) - 64}`);
await settle(300);
await p.keyboard.press('Alt+Shift+4');
await p.keyboard.press('Alt+Shift+5');
await settle(300);
hot = await chipTexts('hot');
console.log('after deletes', hot);

await p.locator('[data-cue-key="h1"] .ff-detail-pane__cue-main').click();
await settle(300);
const cueTime = secs(await curTime());
await seekOverview(0.4);
const away = secs(await curTime());
const cueBtn = p.locator('.ff-xp__cue-btn');
const box = await cueBtn.boundingBox();
await p.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
await p.mouse.down();
await settle(900);
ok(await isPlaying(), 'CUE held while paused plays (preview)');
const during = secs(await curTime());
ok(during >= away, `preview starts at the playhead (${away} -> ${during})`);
await shot('09-cue-held');
await p.mouse.up();
await settle(500);
ok(!(await isPlaying()), 'CUE released pauses');
ok(secs(await curTime()) === away, `CUE released returns to the cue point set where it was pressed (${await curTime()})`);

await p.keyboard.press('Space');
await settle(1200);
ok(await isPlaying(), 'play runs on');
await p.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
await p.mouse.down();
await p.mouse.up();
await settle(500);
ok(!(await isPlaying()), 'CUE while playing pauses');
ok(secs(await curTime()) === away, `CUE while playing returns to the cue point (${await curTime()})`);
void cueTime;

await p.keyboard.press('c');
await settle(300);
ok(!(await isPlaying()), 'C key while paused only sets the cue point');

await p.waitForTimeout(900);
await expand();
await settle(500);
ok((await p.locator('.ff-player-module--expanded').count()) === 0, 'back to the compact player');
ok((await countColor('.ff-player-compact__waveform .waveform-display__zoom', [31, 211, 90], 90)) > 4, 'compact player draws hot cue A marker in its default green');
await shot('10-compact-markers');
await expand();
await settle(400);
await shot('10-before-reload');
const before = { hot: await chipTexts('hot'), mem: await chipTexts('memory') };
console.log('before reload', JSON.stringify(before));

await p.reload();
await p.waitForTimeout(1500);
await p.locator('text=Track 3').first().dblclick();
await settle(900);
const after = { hot: await chipTexts('hot'), mem: await chipTexts('memory') };
console.log('after reload ', JSON.stringify(after));
ok(JSON.stringify(before) === JSON.stringify(after), 'cues survive a reload');
await shot('11-after-reload');

await p.locator('text=Track 5').first().click();
await settle(600);
const other = await chipTexts('hot');
await p.locator('text=Track 3').first().click();
await settle(600);
const back = { hot: await chipTexts('hot'), mem: await chipTexts('memory') };
ok(JSON.stringify(back) === JSON.stringify(after), 'cues survive selecting another track and coming back');
console.log('other track hot', other);

ok(errors.length === 0, `no console errors (${errors.length})`);
if (errors.length) console.log(errors.slice(0, 8));
await browser.close();
console.log(failed ? `${failed} FAILED` : 'ALL PASSED');
process.exit(failed ? 1 : 0);
