import { createRequire } from 'node:module';

/**
 * The scrolled view painted through the column ring (opaque layers, `ground`) must equal a fresh paint of the same view on a background fill, pixel for pixel,
 * after scrolling right, left and across the ring's wrap point.
 *
 *   PW_DIR=<dir with playwright> node perf/scroll-ring-equal.mjs [base url]
 */
const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [base = 'http://localhost:5210/'] = process.argv.slice(2);
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 800, height: 600 } });
await page.goto(base, { waitUntil: 'load' });
const result = await page.evaluate(async () => {
  const url = performance.getEntriesByType('resource').map((r) => r.name).find((n) => /waveform-render\.js/.test(n));
  const { drawBandRange, wavePalette } = await import(/* @vite-ignore */ url.replace(/\?.*$/, ''));
  const columns = 6000;
  const bands = {
    low: Float32Array.from({ length: columns }, (_, i) => 0.5 + 0.4 * Math.sin(i / 17)),
    mid: Float32Array.from({ length: columns }, (_, i) => 0.4 + 0.3 * Math.sin(i / 5)),
    high: Float32Array.from({ length: columns }, (_, i) => 0.3 + 0.3 * Math.sin(i / 3)),
  };
  const width = 300;
  const height = 80;
  const palette = wavePalette();
  const make = () => {
    const canvas = document.createElement('canvas');
    canvas.width = width;
    canvas.height = height;
    return canvas;
  };
  const draw = (canvas, startFrac, ground) => {
    const ctx = canvas.getContext('2d');
    if (!ground) {
      ctx.fillStyle = palette.bg;
      ctx.fillRect(0, 0, width, height);
    }
    drawBandRange(ctx, bands, { startFrac, endFrac: startFrac + 0.04, widthPx: width, heightPx: height, palette, dpr: 1, scrolling: true, decay: 0, zoomLook: null, ground: ground ? palette.bg : null });
  };
  const scrolled = make();
  const path = [];
  for (let i = 0; i < 40; i++) path.push(0.3 + i * 0.0009);
  for (let i = 0; i < 25; i++) path.push(0.3 + 39 * 0.0009 - i * 0.0013);
  for (let i = 0; i < 40; i++) path.push(0.3 + 0.0001 * i * 11);
  let differing = 0;
  let checked = 0;
  for (const start of path) {
    draw(scrolled, start, true);
    const fresh = make();
    draw(fresh, start, false);
    const a = scrolled.getContext('2d').getImageData(0, 0, width, height).data;
    const b = fresh.getContext('2d').getImageData(0, 0, width, height).data;
    checked++;
    for (let i = 0; i < a.length; i++) {
      if (a[i] !== b[i]) {
        differing++;
        break;
      }
    }
  }
  return { checked, differing };
});
console.log(JSON.stringify(result));
await browser.close();
process.exit(result.differing === 0 ? 0 : 1);
