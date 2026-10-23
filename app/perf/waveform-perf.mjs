import { createRequire } from 'node:module';
import { readdirSync, readFileSync } from 'node:fs';

const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [base = 'http://127.0.0.1:4313/', count = '30000', counting = ''] = process.argv.slice(2);

const pct = (arr, p) => {
  if (!arr.length) return 0;
  const s = [...arr].sort((a, b) => a - b);
  return s[Math.min(s.length - 1, Math.floor((p / 100) * s.length))];
};
const round = (n) => Math.round(n * 100) / 100;

function descendants(root) {
  const parent = new Map();
  for (const name of readdirSync('/proc')) {
    if (!/^\d+$/.test(name)) continue;
    try {
      const stat = readFileSync(`/proc/${name}/stat`, 'utf8');
      parent.set(Number(name), Number(stat.slice(stat.lastIndexOf(')') + 2).split(' ')[1]));
    } catch {}
  }
  const out = [];
  for (const [pid] of parent) {
    let p = pid;
    while (p && p !== root) p = parent.get(p);
    if (p === root && pid !== root) out.push(pid);
  }
  return out;
}

function rendererMainCpuMs() {
  let best = 0;
  for (const pid of descendants(process.pid)) {
    try {
      if (!readFileSync(`/proc/${pid}/cmdline`, 'utf8').includes('--type=renderer')) continue;
      const ns = Number(readFileSync(`/proc/${pid}/task/${pid}/schedstat`, 'utf8').split(' ')[0]);
      best = Math.max(best, ns / 1e6);
    } catch {}
  }
  return best;
}

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const errors = [];
page.on('console', (m) => {
  if (m.type() === 'error') errors.push(`console: ${m.text()}`);
});
page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));

await page.addInitScript((doCount) => {
  window.__longTasks = [];
  new PerformanceObserver((list) => {
    for (const e of list.getEntries()) window.__longTasks.push(e.duration);
  }).observe({ entryTypes: ['longtask'] });
  window.__calls = { getComputedStyle: 0, getBoundingClientRect: 0, getContext: 0, canvasDraws: 0 };
  if (doCount) {
    const gcs = window.getComputedStyle;
    window.getComputedStyle = function (...a) {
      window.__calls.getComputedStyle++;
      return gcs.apply(this, a);
    };
    const gbr = Element.prototype.getBoundingClientRect;
    Element.prototype.getBoundingClientRect = function () {
      window.__calls.getBoundingClientRect++;
      return gbr.call(this);
    };
    const fr = CanvasRenderingContext2D.prototype.fillRect;
    CanvasRenderingContext2D.prototype.fillRect = function (...a) {
      window.__calls.canvasDraws++;
      return fr.apply(this, a);
    };
    const di = CanvasRenderingContext2D.prototype.drawImage;
    CanvasRenderingContext2D.prototype.drawImage = function (...a) {
      window.__calls.canvasDraws++;
      return di.apply(this, a);
    };
  }
}, counting === 'count');

const client = await page.context().newCDPSession(page);
await client.send('Performance.enable');
await page.goto(`${base}?mock=${count}`, { waitUntil: 'commit', timeout: 0 });
await page.waitForSelector('[data-track-row]', { timeout: 900000 });
await page.waitForTimeout(3000);

const metrics = async () =>
  Object.fromEntries((await client.send('Performance.getMetrics')).metrics.map((m) => [m.name, m.value]));

async function window_(label, run) {
  await page.evaluate(() => {
    window.__longTasks.length = 0;
    for (const k of Object.keys(window.__calls)) window.__calls[k] = 0;
  });
  const m0 = await metrics();
  const cpu0 = rendererMainCpuMs();
  const out = await run();
  const cpu1 = rendererMainCpuMs();
  const m1 = await metrics();
  const long = await page.evaluate(() => [...window.__longTasks]);
  const calls = await page.evaluate(() => ({ ...window.__calls }));
  const frames = out.deltas.slice(2);
  const secs = m1.Timestamp - m0.Timestamp;
  return {
    [label]: {
      frames: frames.length,
      frameP50: round(pct(frames, 50)),
      frameP95: round(pct(frames, 95)),
      frameP99: round(pct(frames, 99)),
      frameMax: round(Math.max(...frames)),
      over33ms: frames.filter((d) => d > 33).length,
      longTasks: long.length,
      longTaskMs: round(long.reduce((a, b) => a + b, 0)),
      longTaskMax: round(Math.max(0, ...long)),
      scriptMsPerSec: round(((m1.ScriptDuration - m0.ScriptDuration) * 1000) / secs),
      layoutMsPerSec: round(((m1.LayoutDuration - m0.LayoutDuration) * 1000) / secs),
      styleMsPerSec: round(((m1.RecalcStyleDuration - m0.RecalcStyleDuration) * 1000) / secs),
      taskMsPerSec: round(((m1.TaskDuration - m0.TaskDuration) * 1000) / secs),
      totalMs: {
        wall: round(secs * 1000),
        mainCpu: round(cpu1 - cpu0),
        task: round((m1.TaskDuration - m0.TaskDuration) * 1000),
        script: round((m1.ScriptDuration - m0.ScriptDuration) * 1000),
        layout: round((m1.LayoutDuration - m0.LayoutDuration) * 1000),
        style: round((m1.RecalcStyleDuration - m0.RecalcStyleDuration) * 1000),
      },
      ...(counting === 'count' ? { calls } : {}),
      ...(out.extra ?? {}),
    },
  };
}

const rafSteps = (stepFn, steps) =>
  page.evaluate(
    ([fnSrc, steps]) =>
      new Promise((resolve) => {
        const step = new Function('el', 'i', fnSrc);
        const el = document.querySelector('.ff-table__scroll');
        const deltas = [];
        let last = performance.now();
        let i = 0;
        const tick = (now) => {
          deltas.push(now - last);
          last = now;
          step(el, i++);
          if (i < steps) requestAnimationFrame(tick);
          else requestAnimationFrame(() => requestAnimationFrame(() => resolve({ deltas })));
        };
        requestAnimationFrame(tick);
      }),
    [stepFn, steps],
  );

const rafLoop = (stepFn, ms) =>
  page.evaluate(
    ([fnSrc, ms]) =>
      new Promise((resolve) => {
        const step = new Function('el', 'i', 't', fnSrc);
        const el = document.querySelector('.ff-table__scroll');
        const deltas = [];
        let last = performance.now();
        const start = last;
        let i = 0;
        const tick = (now) => {
          deltas.push(now - last);
          last = now;
          step(el, i++, now - start);
          if (now - start < ms) requestAnimationFrame(tick);
          else resolve({ deltas });
        };
        requestAnimationFrame(tick);
      }),
    [stepFn, ms],
  );

const result = { tracks: Number(count) };

Object.assign(result, await window_('scrollFast', async () => rafLoop('el.scrollTop += 90', 3000)));
await page.evaluate(() => (document.querySelector('.ff-table__scroll').scrollTop = 0));
await page.waitForTimeout(500);

Object.assign(result, await window_('scrollSteps120', async () => rafSteps('el.scrollTop += 90', 120)));
await page.evaluate(() => (document.querySelector('.ff-table__scroll').scrollTop = 0));
await page.waitForTimeout(500);

Object.assign(
  result,
  await window_('flingSteps20', async () =>
    rafSteps('el.scrollTop = (el.scrollHeight - el.clientHeight) * ((i + 1) / 20)', 20),
  ),
);
await page.evaluate(() => (document.querySelector('.ff-table__scroll').scrollTop = 0));
await page.waitForTimeout(500);

Object.assign(
  result,
  await window_('scrollFling', async () =>
    rafLoop('el.scrollTop = (el.scrollHeight - el.clientHeight) * Math.min(1, t / 1500)', 1800),
  ),
);
const endState = await page.evaluate(() => {
  const el = document.querySelector('.ff-table__scroll');
  return { top: el.scrollTop, max: el.scrollHeight - el.clientHeight };
});
result.scrollFling.reachedEnd = endState.top >= endState.max - 1;
await page.evaluate(() => (document.querySelector('.ff-table__scroll').scrollTop = 0));
await page.waitForTimeout(500);

if (process.env.ONLY === 'list') {
  result.errors = errors;
  console.log(JSON.stringify(result, null, 1));
  await browser.close();
  process.exit(0);
}

await page.evaluate(
  () =>
    new Promise((resolve) => {
      document.querySelectorAll('[data-track-row]')[13].dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true }));
      const poll = () => (performance.getEntriesByName('ff:player-ready').length ? resolve() : setTimeout(poll, 0));
      poll();
    }),
);
await page.waitForTimeout(1500);
const playLabel = await page.locator('.ff-player-compact__play-btn').getAttribute('aria-label');
if (playLabel !== 'Pause') await page.locator('.ff-player-compact__play-btn').click();
await page.waitForTimeout(800);

const idleFrames = (ms) =>
  page.evaluate(
    (ms) =>
      new Promise((resolve) => {
        const deltas = [];
        let last = performance.now();
        const start = last;
        const tick = (now) => {
          deltas.push(now - last);
          last = now;
          if (now - start < ms) requestAnimationFrame(tick);
          else resolve({ deltas });
        };
        requestAnimationFrame(tick);
      }),
    ms,
  );

Object.assign(result, await window_('playerCompactPlaying', () => idleFrames(4000)));

await page.locator('.ff-player-compact__expand-btn').click();
await page.waitForTimeout(800);
Object.assign(result, await window_('playerExpandedPlaying', () => idleFrames(4000)));

Object.assign(
  result,
  await window_('expandedZoom', async () => {
    const out = await page.evaluate(
      () =>
        new Promise((resolve) => {
          const deltas = [];
          let last = performance.now();
          const start = last;
          const seq = ['Zoom in', 'Zoom in', 'Zoom in', 'Zoom out', 'Zoom out', 'Reset zoom', 'Zoom in'];
          let i = 0;
          const tick = (now) => {
            deltas.push(now - last);
            last = now;
            if (i % 20 === 0 && i / 20 < seq.length) document.querySelector(`[aria-label="${seq[i / 20]}"]`).click();
            i++;
            if (now - start < 3000) requestAnimationFrame(tick);
            else resolve({ deltas });
          };
          requestAnimationFrame(tick);
        }),
    );
    return out;
  }),
);

const box = await page.locator('.waveform-display__zoom').boundingBox();
await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
Object.assign(
  result,
  await window_('expandedWheelScroll', async () => {
    const p = idleFrames(2500);
    for (let i = 0; i < 40; i++) {
      await page.mouse.wheel(i % 2 ? 120 : -120, 0);
      await page.waitForTimeout(50);
    }
    return p;
  }),
);

result.errors = errors;
console.log(JSON.stringify(result, null, 1));
await browser.close();
