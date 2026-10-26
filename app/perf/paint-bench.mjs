import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import { extname } from 'node:path';

/**
 * Full-repaint cost of the zoom view painter, two design-system folders side by side:
 *   PW_DIR=<playwright dir> node perf/paint-bench.mjs <old design-system dir> <new design-system dir> [dpr=2]
 * [dpr] and [columns per pixel] default to 2 and 0.3. Every iteration jumps further than the canvas is wide, so the incremental scroll cache never helps.
 */
const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');
const [oldDir, newDir, dprArg = '2', cppArg = '0.3'] = process.argv.slice(2);
const dirs = { old: oldDir.replace(/\/?$/, '/'), new: newDir.replace(/\/?$/, '/') };
const MIME = { '.js': 'text/javascript', '.css': 'text/css', '.html': 'text/html' };

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 400, height: 300 }, deviceScaleFactor: Number(dprArg) });
await page.route('http://ds.test/**', (route) => {
  const path = new URL(route.request().url()).pathname;
  const [, side, ...rest] = path.split('/');
  if (!side) return route.fulfill({ contentType: 'text/html', body: '<!doctype html><body></body>' });
  return route.fulfill({ contentType: MIME[extname(path)] ?? 'text/plain', body: readFileSync(dirs[side] + rest.join('/')) });
});
await page.goto('http://ds.test/');
const result = await page.evaluate(
  async ({ dpr, cpp }) => {
    const out = {};
    const columns = 40000;
    const synth = () => {
      const make = (f1, f2, f3, amp) => Float32Array.from({ length: columns }, (_, i) => Math.min(1, amp * (0.55 + 0.45 * Math.sin(i * f1)) * (0.4 + 0.6 * Math.abs(Math.sin(i * f2 + f3)))));
      return { low: make(0.004, 0.31, 0, 0.9), mid: make(0.006, 0.17, 1, 0.8), high: make(0.01, 0.09, 2, 0.7) };
    };
    for (const side of ['old', 'new', 'old', 'new']) {
      const render = await import(`/${side}/waveform-render.js`);
      const style = document.documentElement.style;
      const palette = {
        bg: '#0d0d0d', low: '#0053e2', mid: '#fda602', high: '#fdfdfd', midInLow: '#b2660a', highInLow: '#ece8e5', highInMid: '#f6e4d2',
        dim: 'rgba(0,0,0,.5)', ink: '255,255,255', playhead: '#fff', version: 0,
      };
      void style;
      const bands = synth();
      const widthPx = 1128 * dpr;
      const heightPx = 156 * dpr;
      const canvas = document.createElement('canvas');
      canvas.width = widthPx;
      canvas.height = heightPx;
      const ctx = canvas.getContext('2d');
      const frames = 120;
      const fracPerPx = cpp / columns;
      const t0 = performance.now();
      for (let f = 0; f < frames; f++) {
        ctx.fillStyle = palette.bg;
        ctx.fillRect(0, 0, widthPx, heightPx);
        const left = (5000 + f * (widthPx + 17) * 0.37) / Math.max(1, cpp);
        const startFrac = left * fracPerPx;
        render.drawBandRange(ctx, bands, { startFrac, endFrac: startFrac + widthPx * fracPerPx, widthPx, heightPx, palette, dpr, decay: 0.86, scrolling: true });
        if (f % 5 === 0) ctx.getImageData(0, 0, 1, 1);
      }
      ctx.getImageData(0, 0, 1, 1);
      (out[side] ??= []).push(((performance.now() - t0) / frames).toFixed(2));
    }
    return out;
  },
  { dpr: Number(dprArg), cpp: Number(cppArg) },
);
console.log(JSON.stringify(result));
await browser.close();
