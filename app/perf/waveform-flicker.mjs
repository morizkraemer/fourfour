import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';

const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [base = 'http://127.0.0.1:4313/', frames = '90', dprs = '1,2'] = process.argv.slice(2);
const FRAME_COUNT = Number(frames);
const FIXTURE = process.env.FIXTURE;
const SCOPE = FIXTURE ? '.cmp-play ' : '';
const MIN_COLUMN_COVERAGE = 0.9;
const MAX_COVERAGE_DROP = 0.1;
const MAX_BODY_LUMA_SWING = 0.03;

const browser = await chromium.launch();
const failures = [];
const report = [];

for (const dpr of dprs.split(',').map(Number)) {
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: dpr });
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
  await page.addInitScript(() => {
    window.__flicker = { widthSets: 0, heightSets: 0, clears: 0 };
    const widthDesc = Object.getOwnPropertyDescriptor(HTMLCanvasElement.prototype, 'width');
    const heightDesc = Object.getOwnPropertyDescriptor(HTMLCanvasElement.prototype, 'height');
    Object.defineProperty(HTMLCanvasElement.prototype, 'width', {
      ...widthDesc,
      set(v) {
        if (this.classList.contains('waveform-display__zoom') || this.classList.contains('waveform-display__overview')) window.__flicker.widthSets++;
        widthDesc.set.call(this, v);
      },
    });
    Object.defineProperty(HTMLCanvasElement.prototype, 'height', {
      ...heightDesc,
      set(v) {
        if (this.classList.contains('waveform-display__zoom') || this.classList.contains('waveform-display__overview')) window.__flicker.heightSets++;
        heightDesc.set.call(this, v);
      },
    });
  });
  if (FIXTURE) {
    const body = readFileSync(FIXTURE);
    await page.route('**/compare-fixture.json', (route) => route.fulfill({ contentType: 'application/json', body }));
    await page.goto(`${base}compare.html?play=1`, { waitUntil: 'load' });
    await page.waitForSelector('body[data-ready="1"]', { timeout: 60000 });
    await page.waitForTimeout(1500);
  } else {
    await page.goto(`${base}?mock=2000`, { waitUntil: 'load' });
    await page.waitForSelector('[data-track-row]', { timeout: 120000 });
    await page.waitForTimeout(1500);
    await page.evaluate(() => {
      document.querySelectorAll('[data-track-row]')[13].dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true }));
    });
    await page.waitForTimeout(1500);
    if ((await page.locator('.ff-player-compact__play-btn').getAttribute('aria-label')) !== 'Pause') {
      await page.locator('.ff-player-compact__play-btn').click();
    }
    await page.locator('.ff-player-compact__expand-btn').click();
    await page.waitForSelector('.waveform-display__zoom');
    await page.waitForTimeout(500);
    const overviewBox = await page.locator('.waveform-display__overview').boundingBox();
    await page.mouse.click(overviewBox.x + overviewBox.width * 0.4, overviewBox.y + overviewBox.height / 2);
    await page.waitForTimeout(1500);
  }

  const result = await page.evaluate(
    ([count, scope]) =>
      new Promise((resolve) => {
        const zoom = document.querySelector(`${scope}.waveform-display__zoom`);
        const overview = document.querySelector(`${scope}.waveform-display__overview`);
        const host = zoom.closest('.waveform-display').parentElement;
        const mutations = [];
        new MutationObserver((list) => {
          for (const m of list) if (m.addedNodes.length || m.removedNodes.length) mutations.push(m.target.className || m.target.nodeName);
        }).observe(host, { childList: true, subtree: true });
        const before = { ...window.__flicker };
        const sample = [];
        const probe = document.createElement('canvas').getContext('2d');
        probe.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--ff-wave-bg').trim() || '#0d0d0d';
        probe.fillRect(0, 0, 1, 1);
        const BACKGROUND = [...probe.getImageData(0, 0, 1, 1).data];

        const inkColumns = (canvas) => {
          const ctx = canvas.getContext('2d', { willReadFrequently: true });
          const w = canvas.width;
          const h = canvas.height;
          const rowTop = Math.floor(h / 2) - 2;
          const data = ctx.getImageData(0, rowTop, w, 5).data;
          const bg = BACKGROUND;
          let inked = 0;
          let left = 0;
          let right = 0;
          for (let x = 0; x < w; x++) {
            let hit = false;
            for (let y = 0; y < 5 && !hit; y++) {
              const i = (y * w + x) * 4;
              if (Math.abs(data[i] - bg[0]) + Math.abs(data[i + 1] - bg[1]) + Math.abs(data[i + 2] - bg[2]) > 12 || data[i + 3] === 0) hit = true;
            }
            if (hit) {
              inked++;
              if (x < w / 2) left++;
              else right++;
            }
          }
          let transparent = 0;
          for (let x = 0; x < w; x += 8) if (data[x * 4 + 3] === 0) transparent++;
          return { w, h, coverage: inked / w, left: left / (w / 2), right: right / (w / 2), transparent: transparent / Math.ceil(w / 8) };
        };

        const profile = (canvas) => {
          const ctx = canvas.getContext('2d', { willReadFrequently: true });
          const w = canvas.width;
          const h = canvas.height;
          const data = ctx.getImageData(0, 0, w, h).data;
          const out = new Uint16Array(w);
          for (let x = 0; x < w; x++) {
            let count = 0;
            for (let y = 0; y < h; y++) {
              const i = (y * w + x) * 4;
              if (Math.abs(data[i] - BACKGROUND[0]) + Math.abs(data[i + 1] - BACKGROUND[1]) + Math.abs(data[i + 2] - BACKGROUND[2]) > 24) count++;
            }
            out[x] = count;
          }
          return Array.from(out);
        };

        const bodyLuma = (canvas) => {
          const ctx = canvas.getContext('2d', { willReadFrequently: true });
          const w = canvas.width;
          const h = canvas.height;
          const x0 = Math.floor(w * 0.55);
          const x1 = Math.floor(w * 0.95);
          const y0 = Math.floor(h * 0.4);
          const rowsCount = Math.max(1, Math.floor(h * 0.2));
          const data = ctx.getImageData(x0, y0, x1 - x0, rowsCount).data;
          let sum = 0;
          for (let i = 0; i < data.length; i += 4) sum += data[i] + data[i + 1] + data[i + 2];
          return sum / (data.length / 4) / 3;
        };

        let n = 0;
        const frame = () => {
          setTimeout(() => {
            sample.push({ zoom: inkColumns(zoom), overview: inkColumns(overview), canvasW: zoom.width, canvasH: zoom.height, profile: profile(zoom), luma: bodyLuma(zoom) });
            if (sample.length >= count) {
              const after = { ...window.__flicker };
              resolve({
                sample,
                widthSets: after.widthSets - before.widthSets,
                heightSets: after.heightSets - before.heightSets,
                domMutations: mutations.length,
                sameCanvas: document.querySelector(`${scope}.waveform-display__zoom`) === zoom,
              });
            }
          }, 0);
          if (++n < count + 5) requestAnimationFrame(frame);
        };
        requestAnimationFrame(frame);
      }),
    [FRAME_COUNT, SCOPE],
  );

  const median = (a) => [...a].sort((x, y) => x - y)[Math.floor(a.length / 2)];
  const zoomCoverage = result.sample.map((s) => s.zoom.coverage);
  const reference = median(zoomCoverage);
  const bad = [];
  result.sample.forEach((s, i) => {
    const reasons = [];
    if (s.zoom.coverage < MIN_COLUMN_COVERAGE) reasons.push(`coverage ${s.zoom.coverage.toFixed(3)}`);
    if (reference - s.zoom.coverage > MAX_COVERAGE_DROP) reasons.push(`drop ${(reference - s.zoom.coverage).toFixed(3)}`);
    if (s.zoom.left < MIN_COLUMN_COVERAGE || s.zoom.right < MIN_COLUMN_COVERAGE) reasons.push(`half-drawn ${s.zoom.left.toFixed(2)}/${s.zoom.right.toFixed(2)}`);
    if (s.zoom.transparent > 0) reasons.push(`transparent ${s.zoom.transparent.toFixed(2)}`);
    if (s.overview.coverage < MIN_COLUMN_COVERAGE || s.overview.transparent > 0) reasons.push(`overview ${s.overview.coverage.toFixed(3)}`);
    if (reasons.length) bad.push({ frame: i, reasons });
  });
  const shimmer = [];
  const shifts = [];
  for (let i = 1; i < result.sample.length; i++) {
    const a = result.sample[i - 1].profile;
    const b = result.sample[i].profile;
    const margin = 40 * dpr;
    let best = Infinity;
    let bestShift = 0;
    for (let shift = 0; shift <= 60 * dpr; shift++) {
      let sum = 0;
      let count = 0;
      for (let x = margin; x < a.length - margin; x += 2) {
        sum += Math.abs(a[x + shift] - b[x]);
        count++;
      }
      if (sum / count < best) {
        best = sum / count;
        bestShift = shift;
      }
    }
    shimmer.push(best / result.sample[0].zoom.h);
    shifts.push(bestShift);
  }
  const lumas = result.sample.map((x) => x.luma);
  const lumaMean = lumas.reduce((a, b) => a + b, 0) / lumas.length;
  const lumaSwing = (Math.max(...lumas) - Math.min(...lumas)) / lumaMean;
  if (lumaSwing > MAX_BODY_LUMA_SWING) failures.push({ dpr, bodyBrightnessSwing: Number(lumaSwing.toFixed(4)) });
  const summary = {
    bodyBrightnessSwing: Number(lumaSwing.toFixed(4)),
    worst: shimmer.map((v, i) => [i + 1, Number(v.toFixed(3)), shifts[i]]).sort((x, y) => y[1] - x[1]).slice(0, 6),
    shimmerMedian: Number(median(shimmer).toFixed(4)),
    shimmerMax: Number(Math.max(...shimmer).toFixed(4)),
    dpr,
    frames: result.sample.length,
    zoomPx: `${result.sample[0].zoom.w}x${result.sample[0].zoom.h}`,
    medianCoverage: Number(reference.toFixed(3)),
    minCoverage: Number(Math.min(...zoomCoverage).toFixed(3)),
    canvasWidthSets: result.widthSets,
    canvasHeightSets: result.heightSets,
    domMutations: result.domMutations,
    sameCanvasElement: result.sameCanvas,
    badFrames: bad.length,
    errors,
  };
  report.push(summary);
  if (bad.length) failures.push({ dpr, bad: bad.slice(0, 10) });
  if (result.widthSets || result.heightSets || result.domMutations || !result.sameCanvas) failures.push({ dpr, resizeOrRebuild: summary });
  if (errors.length) failures.push({ dpr, errors });
  await page.close();
}

await browser.close();
console.log(JSON.stringify({ report, failures }, null, 1));
process.exitCode = failures.length ? 1 : 0;
