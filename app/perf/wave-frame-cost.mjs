import { createRequire } from 'node:module';

/**
 * Frame cost of the expanded player's scrolling waveform while playing (mock transport).
 *
 *   PW_DIR=<dir with playwright> node perf/wave-frame-cost.mjs [base url] [seconds] [zoomInClicks] [cpuThrottle]
 *
 * Every animation frame is timed from its rAF timestamp to the end of the task (next frame's start is not used):
 * `work` is the main-thread time spent in the frame callbacks, `gap` the time between two frames. Frames whose gap is
 * more than 1.5 display intervals (a dropped frame) are counted, and the CPU profile's top self-time entries printed.
 */
const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [base = 'http://127.0.0.1:5210/', secondsArg = '8', zoomInArg = '0', throttleArg = '1'] = process.argv.slice(2);
const SECONDS = Number(secondsArg);

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1600, height: 1000 }, deviceScaleFactor: Number(process.env.DPR ?? 1) });
const errors = [];
page.on('pageerror', (e) => errors.push(e.message));
const client = await page.context().newCDPSession(page);
await page.addInitScript(() => {
  window.__work = { raf: [], timer: [] };
  const raf = window.requestAnimationFrame.bind(window);
  window.requestAnimationFrame = (cb) =>
    raf((t) => {
      const begin = performance.now();
      cb(t);
      window.__work.raf.push({ t, ms: performance.now() - begin });
    });
  window.__ops = { calls: {}, pixels: 0 };
  const proto = CanvasRenderingContext2D.prototype;
  for (const name of ['drawImage', 'fillRect', 'clearRect', 'stroke', 'fillText', 'strokeRect', 'fill', 'putImageData']) {
    const original = proto[name];
    proto[name] = function (...args) {
      window.__ops.calls[name] = (window.__ops.calls[name] ?? 0) + 1;
      const t = this.getTransform();
      const scale = Math.abs(t.a * t.d - t.b * t.c);
      if (name === 'fillRect' || name === 'clearRect' || name === 'strokeRect') window.__ops.pixels += Math.abs(args[2] * args[3]) * scale;
      else if (name === 'drawImage') {
        const src = args[0];
        window.__ops.pixels += args.length >= 5 ? Math.abs(args[3] * args[4]) * scale : src.width * src.height * scale;
      }
      return original.apply(this, args);
    };
  }
  const interval = window.setInterval.bind(window);
  window.setInterval = (fn, ms, ...rest) =>
    interval(
      typeof fn === 'function'
        ? (...args) => {
            const begin = performance.now();
            fn(...args);
            window.__work.timer.push({ t: begin, ms: performance.now() - begin });
          }
        : fn,
      ms,
      ...rest,
    );
});
await page.goto(`${base}?mock=${process.env.MOCK ?? 2000}`, { waitUntil: 'load' });
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
if (Number(throttleArg) > 1) await client.send('Emulation.setCPUThrottlingRate', { rate: Number(throttleArg) });
await page.waitForTimeout(500);

if (process.env.BREAKDOWN) {
  await page.evaluate(async () => {
    const url = performance.getEntriesByType('resource').map((r) => r.name).find((n) => /waveform-display\.js/.test(n));
    const mod = await import(/* @vite-ignore */ url.replace(/\?.*$/, ''));
    window.__parts = {};
    const proto = mod.default.prototype;
    for (const name of ['_paint', '_renderOverview', '_renderZoom', '_renderBands', '_renderBeatGrid', '_renderTimeGrid', '_renderCues', '_updatePlayheadOverlay', 'setTransport', 'setCues', 'setBeats', 'setData', 'setDetailBands', '_applyPlayhead', '_stepInterp', '_fetchWindow', '_zoomWindow']) {
      const original = proto[name];
      if (!original) continue;
      proto[name] = function (...args) {
        const begin = performance.now();
        const result = original.apply(this, args);
        const list = (window.__parts[name] ??= []);
        list.push(performance.now() - begin);
        return result;
      };
    }
  });
}
await page.evaluate(() => {
  window.__ops.calls = {};
  window.__ops.pixels = 0;
  window.__work.raf.length = 0;
  window.__work.timer.length = 0;
});
const traced = [];
if (process.env.TRACE) {
  client.on('Tracing.dataCollected', (e) => traced.push(...e.value));
  await client.send('Tracing.start', { categories: 'devtools.timeline,v8,disabled-by-default-v8.gc', transferMode: 'ReportEvents' });
}
await client.send('Profiler.enable');
await client.send('Profiler.setSamplingInterval', { interval: 200 });
await client.send('Profiler.start');
const frames = await page.evaluate(async (seconds) => {
  const out = [];
  const start = performance.now();
  let last = null;
  await new Promise((resolve) => {
    function frame(t) {
      const begin = performance.now();
      out.push({ t, begin, last });
      last = t;
      if (t - start < seconds * 1000) requestAnimationFrame(frame);
      else resolve();
    }
    requestAnimationFrame(frame);
  });
  return out.map((f, i) => ({ gap: f.last === null ? 0 : f.t - f.last, begin: f.begin, t: f.t, i }));
}, SECONDS);
const { profile } = await client.send('Profiler.stop');
const work = await page.evaluate(() => window.__work);
const ops = await page.evaluate(() => window.__ops);
const parts = process.env.BREAKDOWN ? await page.evaluate(() => window.__parts) : {};
if (process.env.TRACE) {
  const done = new Promise((resolve) => client.once('Tracing.tracingComplete', resolve));
  await client.send('Tracing.end');
  await done;
  const byName = new Map();
  for (const e of traced) {
    if (e.ph !== 'X' || !e.dur || e.dur < 4000) continue;
    const entry = byName.get(e.name) ?? { n: 0, total: 0, max: 0 };
    entry.n++;
    entry.total += e.dur / 1000;
    entry.max = Math.max(entry.max, e.dur / 1000);
    byName.set(e.name, entry);
  }
  console.log('trace events >4ms', JSON.stringify([...byName.entries()].map(([k, v]) => [k, v.n, Math.round(v.total), Math.round(v.max)]).sort((a, b) => b[2] - a[2]).slice(0, 14)));
}

const gaps = frames.slice(2).map((f) => f.gap);
const sorted = [...gaps].sort((a, b) => a - b);
const pct = (p) => sorted[Math.min(sorted.length - 1, Math.floor((p / 100) * sorted.length))];
const interval = pct(50);
const round = (n) => Math.round(n * 100) / 100;
const dropped = gaps.filter((g) => g > interval * 1.5).length;
const lost = gaps.reduce((a, g) => a + Math.max(0, Math.round(g / interval) - 1), 0);
console.log(
  JSON.stringify({
    frames: gaps.length,
    intervalMs: round(interval),
    gapP95: round(pct(95)),
    gapP99: round(pct(99)),
    gapMax: round(sorted[sorted.length - 1]),
    droppedFrames: dropped,
    framesLost: lost,
    droppedPercent: round((dropped / gaps.length) * 100),
    errors,
  }),
);

for (const [name, list] of Object.entries(parts)) {
  const sortedParts = [...list].sort((a, b) => a - b);
  const at = (p) => sortedParts[Math.min(sortedParts.length - 1, Math.floor((p / 100) * sortedParts.length))];
  console.log(`${name.padEnd(24)} n=${String(list.length).padEnd(5)} p50=${round(at(50))} p95=${round(at(95))} p99=${round(at(99))} max=${round(sortedParts[sortedParts.length - 1])} total=${Math.round(list.reduce((a, b) => a + b, 0))}`);
}
const perFrameOps = Object.fromEntries(Object.entries(ops.calls).map(([k, v]) => [k, round(v / frames.length)]));
console.log(JSON.stringify({ canvasCallsPerFrame: perFrameOps, canvasMegapixelsPerFrame: round(ops.pixels / frames.length / 1e6) }));
const perFrame = new Map();
for (const w of work.raf) perFrame.set(w.t, (perFrame.get(w.t) ?? 0) + w.ms);
const costs = [...perFrame.values()].sort((a, b) => a - b);
const cpct = (p) => costs[Math.min(costs.length - 1, Math.floor((p / 100) * costs.length))];
const ticks = work.timer.map((w) => w.ms).sort((a, b) => a - b);
console.log(
  JSON.stringify({
    rafWorkMsP50: round(cpct(50)),
    rafWorkMsP95: round(cpct(95)),
    rafWorkMsP99: round(cpct(99)),
    rafWorkMsMax: round(costs[costs.length - 1]),
    over8ms: costs.filter((c) => c > 8).length,
    timerCallbacks: ticks.length,
    timerMsMax: round(ticks[ticks.length - 1] ?? 0),
  }),
);

const self = new Map();
const byId = new Map(profile.nodes.map((n) => [n.id, n]));
profile.samples.forEach((id, i) => {
  const f = byId.get(id).callFrame;
  const key = `${f.functionName || '(anon)'} ${f.url.split('/').pop().split('?')[0]}:${f.lineNumber}`;
  self.set(key, (self.get(key) ?? 0) + profile.timeDeltas[i] / 1000);
});
const total = [...self.values()].reduce((a, b) => a + b, 0);
console.log(`profile total ${round(total)} ms over ${SECONDS}s`);
for (const [k, v] of [...self.entries()].sort((a, b) => b[1] - a[1]).slice(0, 16)) console.log(v.toFixed(1).padStart(9), 'ms', k);
await browser.close();
