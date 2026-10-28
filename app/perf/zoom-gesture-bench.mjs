import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';

/**
 * Frame cost of a zoom gesture in the expanded waveform (`zoom-detail.html`): 240 frames from 5 s down to 4 ms per
 * screen and back, one zoom step per animation frame, with the fine windows served by `waveform_window_dump serve`.
 * Reports the paint time per frame (p50, p95, max), frames that missed the 16.7 ms budget, and how many deep frames had
 * no window to draw (they fall back to the wide 2400 columns per second bars).
 *
 * PW_DIR=<dir with playwright> node app/perf/zoom-gesture-bench.mjs BASE FIXTURES NAME AT_S WINDOW_SERVER [CPU_THROTTLE]
 */
const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');
const [base, fixtures, name, at, windowServer, throttle = '4'] = process.argv.slice(2);

const browser = await chromium.launch({ executablePath: process.env.CHROME || undefined });
const context = await browser.newContext({ viewport: { width: 1200, height: 300 }, deviceScaleFactor: 2 });
await context.route('**/fx/*', (route) => {
  const file = new URL(route.request().url()).pathname.replace('/fx/', '');
  try {
    route.fulfill({ body: readFileSync(`${fixtures}/${file}`), contentType: 'application/octet-stream' });
  } catch {
    route.fulfill({ status: 404, body: '' });
  }
});
await context.route('**/win/**', async (route) => {
  const url = new URL(route.request().url());
  const response = await fetch(`${windowServer}/window${url.search}`);
  route.fulfill({ body: Buffer.from(await response.arrayBuffer()), contentType: 'application/octet-stream' });
});
const page = await context.newPage();
const errors = [];
page.on('pageerror', (e) => errors.push(String(e)));
await page.goto(`${base}/zoom-detail.html?track=${name}&win=5&at=${at}&windows=1`);
await page.waitForSelector('body[data-ready="1"]');
const cdp = await context.newCDPSession(page);
await cdp.send('Emulation.setCPUThrottlingRate', { rate: Number(throttle) });
await page.waitForTimeout(500);

if (process.env.PROFILE) {
  await cdp.send('Profiler.enable');
  await cdp.send('Profiler.setSamplingInterval', { interval: 200 });
  await cdp.send('Profiler.start');
}
const result = await page.evaluate(async () => {
  const canvas = document.querySelector('.waveform-display__zoom');
  const display = window.__display;
  const durationMs = display._data.duration_ms;
  const zoomAt = (spanMs) => durationMs / spanMs;
  const frames = 240;
  const times = [];
  let deepWithoutWindow = 0;
  let deepFrames = 0;
  let windowsDrawn = 0;
  const paintWindow = display._paintWindow.bind(display);
  let drewWindow = false;
  display._paintWindow = (...args) => {
    drewWindow = true;
    return paintWindow(...args);
  };
  for (let i = 0; i < frames; i++) {
    const t = i < frames / 2 ? i / (frames / 2 - 1) : (frames - 1 - i) / (frames / 2 - 1);
    const spanMs = 5000 * Math.pow(4 / 5000, t);
    await new Promise((resolve) => requestAnimationFrame(resolve));
    drewWindow = false;
    const t0 = performance.now();
    display._applyZoom(zoomAt(spanMs));
    if (display._zoomRafId) cancelAnimationFrame(display._zoomRafId);
    display._zoomRafId = null;
    display._paint();
    canvas.getContext('2d').getImageData(0, 0, 1, 1);
    times.push(performance.now() - t0);
    const msPerDevicePx = spanMs / canvas.width;
    if (msPerDevicePx < 1000 / 2400) {
      deepFrames++;
      if (drewWindow) windowsDrawn++;
      else deepWithoutWindow++;
    }
  }
  const sorted = [...times].sort((a, b) => a - b);
  const pct = (p) => sorted[Math.min(sorted.length - 1, Math.floor((p / 100) * sorted.length))];
  return {
    p50: +pct(50).toFixed(2), p95: +pct(95).toFixed(2), max: +sorted[sorted.length - 1].toFixed(2),
    overBudget: times.filter((t) => t > 16.7).length, frames, deepFrames, deepWithoutWindow,
    windowsArrived: Number(document.body.dataset.windows || 0),
  };
});
if (process.env.PROFILE) {
  const { profile } = await cdp.send('Profiler.stop');
  const self = new Map();
  const byId = new Map(profile.nodes.map((n) => [n.id, n]));
  const dt = profile.timeDeltas;
  profile.samples.forEach((id, i) => {
    const node = byId.get(id);
    const key = `${node.callFrame.functionName || '(anon)'} ${node.callFrame.url.split('/').pop()}:${node.callFrame.lineNumber}`;
    self.set(key, (self.get(key) || 0) + (dt[i] || 0) / 1000);
  });
  console.log([...self].sort((a, b) => b[1] - a[1]).slice(0, 15).map(([k, v]) => `${v.toFixed(0).padStart(6)} ms ${k}`).join('\n'));
}
console.log(JSON.stringify({ throttle: Number(throttle), ...result, errors }));
await browser.close();
