import { createRequire } from 'node:module';
import { readdirSync, readFileSync } from 'node:fs';

const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [base = 'http://127.0.0.1:4313/', count = '30000', artworkMs = '12', runs = '5'] = process.argv.slice(2);
const SCROLL_MS = 3000;
const DRAG_MS = 3000;

const round = (n) => Math.round(n * 100) / 100;
const median = (a) => [...a].sort((x, y) => x - y)[Math.floor(a.length / 2)];

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

async function once() {
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  const errors = [];
  page.on('console', (m) => {
    if (m.type() === 'error') errors.push(m.text());
  });
  page.on('pageerror', (e) => errors.push(e.message));

  await page.addInitScript(() => {
    const c = (window.__c = {
      invoke: 0,
      artworkFetch: 0,
      objectUrls: 0,
      domAdded: 0,
      domRemoved: 0,
      attrMutations: 0,
      textMutations: 0,
      getBoundingClientRect: 0,
      elementFromPoint: 0,
      getComputedStyle: 0,
      canvasDraws: 0,
      getContext: 0,
      scrollEvents: 0,
      pointermoves: 0,
    });
    let internals;
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      configurable: true,
      get() {
        if (!/getIsTauri/.test(new Error().stack)) c.invoke++;
        return internals;
      },
      set(v) {
        internals = v;
      },
    });
    const ctb = OffscreenCanvas.prototype.convertToBlob;
    OffscreenCanvas.prototype.convertToBlob = function (...a) {
      c.artworkFetch++;
      return ctb.apply(this, a);
    };
    const cou = URL.createObjectURL;
    URL.createObjectURL = function (...a) {
      c.objectUrls++;
      return cou.apply(this, a);
    };
    const gbr = Element.prototype.getBoundingClientRect;
    Element.prototype.getBoundingClientRect = function () {
      c.getBoundingClientRect++;
      return gbr.call(this);
    };
    const efp = Document.prototype.elementFromPoint;
    Document.prototype.elementFromPoint = function (...a) {
      c.elementFromPoint++;
      return efp.apply(this, a);
    };
    const gcs = window.getComputedStyle;
    window.getComputedStyle = function (...a) {
      c.getComputedStyle++;
      return gcs.apply(this, a);
    };
    const gc = HTMLCanvasElement.prototype.getContext;
    HTMLCanvasElement.prototype.getContext = function (...a) {
      c.getContext++;
      return gc.apply(this, a);
    };
    for (const m of ['fillRect', 'drawImage']) {
      const orig = CanvasRenderingContext2D.prototype[m];
      CanvasRenderingContext2D.prototype[m] = function (...a) {
        c.canvasDraws++;
        return orig.apply(this, a);
      };
    }
    window.addEventListener('pointermove', () => c.pointermoves++, true);
    document.addEventListener('scroll', () => c.scrollEvents++, true);
    addEventListener('DOMContentLoaded', () => {
      new MutationObserver((list) => {
        for (const r of list) {
          if (r.type === 'childList') {
            for (const n of r.addedNodes) if (n.nodeType === 1) c.domAdded++;
            for (const n of r.removedNodes) if (n.nodeType === 1) c.domRemoved++;
          } else if (r.type === 'attributes') c.attrMutations++;
          else c.textMutations++;
        }
      }).observe(document.body, {
        subtree: true,
        childList: true,
        attributes: true,
        characterData: true,
      });
    });
  });

  await page.goto(`${base}?mock=${count}&artwork=${artworkMs}`, { waitUntil: 'commit', timeout: 0 });
  await page.waitForSelector('[data-track-row]', { timeout: 900000 });
  await page.waitForTimeout(2500);

  const session = await page.context().newCDPSession(page);
  await session.send('Performance.enable');
  const metrics = async () =>
    Object.fromEntries((await session.send('Performance.getMetrics')).metrics.map((m) => [m.name, m.value]));

  async function measure(run) {
    await page.evaluate(() => {
      for (const k of Object.keys(window.__c)) window.__c[k] = 0;
      window.__frames = 0;
      const tick = () => {
        window.__frames++;
        window.__raf = requestAnimationFrame(tick);
      };
      window.__raf = requestAnimationFrame(tick);
    });
    const m0 = await metrics();
    const cpu0 = rendererMainCpuMs();
    await run();
    const cpu1 = rendererMainCpuMs();
    const m1 = await metrics();
    const { c, frames } = await page.evaluate(() => {
      cancelAnimationFrame(window.__raf);
      return { c: { ...window.__c }, frames: window.__frames };
    });
    const rafFrames = Math.max(frames, 1);
    const per = (n) => round(n / rafFrames);
    return {
      rafFrames,
      cpuMsPerFrame: round((cpu1 - cpu0) / rafFrames),
      scriptMsPerFrame: round(((m1.ScriptDuration - m0.ScriptDuration) * 1000) / rafFrames),
      layoutMsPerFrame: round(((m1.LayoutDuration - m0.LayoutDuration) * 1000) / rafFrames),
      styleMsPerFrame: round(((m1.RecalcStyleDuration - m0.RecalcStyleDuration) * 1000) / rafFrames),
      totals: c,
      perFrame: Object.fromEntries(Object.entries(c).map(([k, v]) => [k, per(v)])),
    };
  }

  const scroll = await measure(() =>
    page.evaluate(
      (ms) =>
        new Promise((resolve) => {
          const el = document.querySelector('.ff-table__scroll');
          const start = performance.now();
          const tick = (now) => {
            el.scrollTop += 90;
            if (now - start < ms) requestAnimationFrame(tick);
            else resolve();
          };
          requestAnimationFrame(tick);
        }),
      SCROLL_MS,
    ),
  );
  await page.evaluate(() => (document.querySelector('.ff-table__scroll').scrollTop = 0));
  await page.waitForTimeout(800);

  const row = page.locator('[data-track-row]').nth(8);
  const box = await row.boundingBox();
  await page.mouse.move(box.x + 300, box.y + 10);
  await page.mouse.down();
  await page.mouse.move(box.x + 310, box.y + 40, { steps: 3 });
  const drag = await measure(async () => {
    const start = Date.now();
    let i = 0;
    while (Date.now() - start < DRAG_MS) {
      const t = (Date.now() - start) / DRAG_MS;
      await page.mouse.move(box.x + 200 + 400 * Math.abs(Math.sin(t * 6)), box.y + 40 + 400 * Math.abs(Math.sin(t * 4 + i * 0)));
      i++;
    }
  });
  await page.mouse.up();
  await page.waitForTimeout(300);
  await browser.close();
  return { scroll, drag, errors };
}

const results = [];
for (let i = 0; i < Number(runs); i++) results.push(await once());

const pick = (key) => ({
  scroll: results.map((r) => r.scroll[key]),
  drag: results.map((r) => r.drag[key]),
});
const out = { tracks: Number(count), artworkMs: Number(artworkMs), runs: results.length };
for (const phase of ['scroll', 'drag']) {
  const rs = results.map((r) => r[phase]);
  out[phase] = {
    rafFrames: median(rs.map((r) => r.rafFrames)),
    cpuMsPerFrame: median(rs.map((r) => r.cpuMsPerFrame)),
    scriptMsPerFrame: median(rs.map((r) => r.scriptMsPerFrame)),
    layoutMsPerFrame: median(rs.map((r) => r.layoutMsPerFrame)),
    styleMsPerFrame: median(rs.map((r) => r.styleMsPerFrame)),
    totals: Object.fromEntries(Object.keys(rs[0].totals).map((k) => [k, median(rs.map((r) => r.totals[k]))])),
    perFrame: Object.fromEntries(Object.keys(rs[0].perFrame).map((k) => [k, median(rs.map((r) => r.perFrame[k]))])),
  };
}
out.errors = results.flatMap((r) => r.errors).slice(0, 5);
console.log(JSON.stringify(out, null, 1));
