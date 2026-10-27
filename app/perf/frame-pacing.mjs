import { createRequire } from 'node:module';
import { existsSync, readFileSync } from 'node:fs';

/**
 * Scroll frame pacing of the expanded player's zoom view while playing (mock transport, 50 ms ticks).
 *
 *   PW_DIR=<dir with playwright> node perf/frame-pacing.mjs [base url] [seconds] [dpr] [zoomInClicks] [jitterMs]
 *
 * The zoom view scrolls by moving its last frame by whole device pixels (`drawScrolledColumns`); every such move
 * is counted. Each animation frame then has a delta in device pixels. Reports steps per second (frames in which the
 * view moved), the distribution of the per-frame deltas, and the instantaneous speed (delta / frame time) spread:
 * smooth scrolling is a constant speed, so its coefficient of variation is small.
 * `jitterMs` delays each 50 ms tick by a random 0..jitterMs, like a busy IPC channel.
 */
const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [base = 'http://127.0.0.1:5200', secondsArg = '6', dprArg = '2', zoomInArg = '0', jitterArg = '0'] = process.argv.slice(2);
const SECONDS = Number(secondsArg);

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: Number(dprArg) });
const errors = [];
page.on('pageerror', (e) => errors.push(e.message));
await page.addInitScript(() => {
  window.__scrolled = { total: 0, paints: 0 };
  const draw = CanvasRenderingContext2D.prototype.drawImage;
  CanvasRenderingContext2D.prototype.drawImage = function (source, x, ...rest) {
    if (source instanceof HTMLCanvasElement && !this.canvas.isConnected && rest.length === 1 && x !== 0) {
      window.__scrolled.total += -x;
      window.__scrolled.paints++;
    }
    return draw.call(this, source, x, ...rest);
  };
});
await page.addInitScript((jitter) => {
  const realSetInterval = window.setInterval.bind(window);
  window.setInterval = (fn, ms, ...rest) =>
    realSetInterval(ms === 50 && jitter > 0 ? () => setTimeout(fn, Math.random() * jitter) : fn, ms, ...rest);
}, Number(jitterArg));
if (process.env.OVERRIDE_DIR) {
  await page.route(/design-system\/waveform-[a-z]+\.js|design-system\/playhead-clock\.js/, async (route) => {
    const name = new URL(route.request().url()).pathname.split('/').pop();
    const file = `${process.env.OVERRIDE_DIR}/${name}`;
    if (!existsSync(file)) return route.continue();
    return route.fulfill({ contentType: 'text/javascript', body: readFileSync(file) });
  });
}
await page.goto(`${base}/?mock=2000`, { waitUntil: 'load' });
await page.waitForSelector('[data-track-row]', { timeout: 120000 });
await page.waitForTimeout(1000);
await page.evaluate(() => {
  document.querySelectorAll('[data-track-row]')[13].dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true }));
});
await page.waitForTimeout(1500);
if ((await page.locator('.ff-player-compact__play-btn').getAttribute('aria-label')) !== 'Pause') {
  await page.locator('.ff-player-compact__play-btn').click();
}
await page.locator('.ff-player-compact__expand-btn').click();
await page.waitForTimeout(800);
for (let i = 0; i < Number(zoomInArg); i++) {
  await page.locator('[aria-label="Zoom in"]').click();
  await page.waitForTimeout(100);
}
await page.waitForTimeout(500);

const result = await page.evaluate(async (seconds) => {
  const reads = [];
  const start = performance.now();
  await new Promise((resolve) => {
    function frame(t) {
      reads.push({ t, n: performance.now(), scrolled: window.__scrolled.total, paints: window.__scrolled.paints });
      if (t - start < seconds * 1000) requestAnimationFrame(frame);
      else resolve();
    }
    requestAnimationFrame(frame);
  });
  const out = [];
  for (let i = 1; i < reads.length; i++) {
    out.push({ dt: reads[i].t - reads[i - 1].t, wall: reads[i].n - reads[i - 1].n, shift: reads[i].scrolled - reads[i - 1].scrolled, paints: reads[i].paints - reads[i - 1].paints });
  }
  return out;
}, SECONDS);

const pct = (arr, p) => arr[Math.min(arr.length - 1, Math.floor((p / 100) * arr.length))];
const shifts = result.map((r) => r.shift);
const dts = result.map((r) => r.dt);
const total = dts.reduce((a, b) => a + b, 0);
const changed = shifts.filter((s) => s !== 0).length;
const hist = {};
for (const s of shifts) hist[s] = (hist[s] ?? 0) + 1;
const mean = shifts.reduce((a, b) => a + b, 0) / shifts.length;
const sd = Math.sqrt(shifts.reduce((a, b) => a + (b - mean) ** 2, 0) / shifts.length);
const velocities = result.map((r) => (r.shift / r.wall) * 1000);
const vMean = velocities.reduce((a, b) => a + b, 0) / velocities.length;
const vSd = Math.sqrt(velocities.reduce((a, b) => a + (b - vMean) ** 2, 0) / velocities.length);
const vSorted = [...velocities].sort((a, b) => a - b);
const sorted = [...dts].sort((a, b) => a - b);
const round = (n, k = 100) => Math.round(n * k) / k;
const summary = {
  frames: result.length,
  seconds: round(total / 1000),
  framesPerSecond: round((result.length / total) * 1000, 10),
  scrollStepsPerSecond: round((changed / total) * 1000, 10),
  stalledFramePercent: round(((result.length - changed) / result.length) * 100, 10),
  deltaMeanPx: round(mean),
  deltaStdPx: round(sd),
  deltaHistogram: hist,
  speedPxPerSecMean: Math.round(vMean),
  speedCoefficientOfVariation: round(vSd / vMean, 1000),
  speedP5: Math.round(pct(vSorted, 5)),
  speedP95: Math.round(pct(vSorted, 95)),
  frameMsP50: round(pct(sorted, 50), 10),
  frameMsP95: round(pct(sorted, 95), 10),
  errors,
};
console.log(JSON.stringify(summary));
if (process.env.TRACE) {
  console.log(result.slice(0, 60).map((r) => `${r.dt.toFixed(1)}:${r.shift}:${r.paints}`).join(' '));
  console.log(JSON.stringify(await page.evaluate(() => window.__lf?.slice(40, 100))));
}
await browser.close();
