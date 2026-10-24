import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import { extname } from 'node:path';

const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const DS = new URL('../../prototyping/viewport/design-system/', import.meta.url).pathname;
const MIME = { '.js': 'text/javascript', '.css': 'text/css', '.html': 'text/html' };

const SEGMENT = 16;
const PATTERN = [
  { low: 0.9, mid: 0, high: 0 },
  { low: 0, mid: 0.9, high: 0 },
  { low: 0, mid: 0, high: 0.9 },
  { low: 0.4, mid: 0.65, high: 0.9 },
];

const browser = await chromium.launch();
const failures = [];
let checks = 0;

for (const dpr of [1, 2]) {
  for (const theme of ['dark', 'light']) {
    const page = await browser.newPage({ viewport: { width: 300, height: 200 }, deviceScaleFactor: dpr });
    await page.route('http://ds.test/**', (route) => {
      const path = new URL(route.request().url()).pathname;
      if (path === '/') {
        return route.fulfill({
          contentType: 'text/html',
          body: '<!doctype html><link rel="stylesheet" href="/tokens.css"><body></body>',
        });
      }
      return route.fulfill({ contentType: MIME[extname(path)] ?? 'text/plain', body: readFileSync(DS + path) });
    });
    await page.goto('http://ds.test/');
    await page.evaluate((t) => {
      if (t === 'light') document.documentElement.dataset.theme = 'light';
    }, theme);

    const result = await page.evaluate(
      async ({ dpr, segment, pattern }) => {
        const render = await import('/waveform-render.js');
        const math = await import('/waveform-bands.js');
        const palette = render.wavePalette();

        const columns = segment * pattern.length;
        const bands = { low: new Float32Array(columns), mid: new Float32Array(columns), high: new Float32Array(columns) };
        pattern.forEach((p, s) => {
          for (let c = 0; c < segment; c++) {
            bands.low[s * segment + c] = p.low;
            bands.mid[s * segment + c] = p.mid;
            bands.high[s * segment + c] = p.high;
          }
        });

        const widthPx = columns * dpr;
        const heightPx = 100 * dpr;
        const ctx = new OffscreenCanvas(widthPx, heightPx).getContext('2d');
        ctx.fillStyle = palette.bg;
        ctx.fillRect(0, 0, widthPx, heightPx);
        render.drawBandRange(ctx, bands, { startFrac: 0, endFrac: 1, widthPx, heightPx, palette, dpr, decay: 0 });
        const image = ctx.getImageData(0, 0, widthPx, heightPx);

        const rgbOf = (css) => {
          const probe = new OffscreenCanvas(1, 1).getContext('2d');
          probe.fillStyle = css;
          probe.fillRect(0, 0, 1, 1);
          return Array.from(probe.getImageData(0, 0, 1, 1).data.slice(0, 3));
        };
        const colours = { bg: rgbOf(palette.bg), low: rgbOf(palette.low), mid: rgbOf(palette.mid), high: rgbOf(palette.high) };
        const pixel = (x, y) => {
          const i = (Math.round(y) * widthPx + x) * 4;
          return Array.from(image.data.slice(i, i + 3));
        };
        const isBg = (c) => c.every((v, k) => Math.abs(v - colours.bg[k]) <= 6);
        const centre = heightPx / 2;
        const half = (band, value) => math.halfHeight(math.displayAmp(value), band, centre);

        return {
          colours,
          centre,
          segments: pattern.map((p, s) => {
            const x = Math.round((s * segment + segment / 2) * dpr);
            let top = centre;
            for (let y = 0; y < centre; y++) if (!isBg(pixel(x, y))) { top = y; break; }
            let bottom = centre;
            for (let y = heightPx - 1; y > centre; y--) if (!isBg(pixel(x, y))) { bottom = y; break; }
            const h = { low: half('low', p.low), mid: half('mid', p.mid), high: half('high', p.high) };
            const above = (height) => pixel(x, centre - height);
            return { top, bottom, half: h, above: {
              inLow: above(h.low * 0.5),
              inMid: above(h.mid * 0.5),
              inHigh: above(h.high * 0.5),
              midRing: above((h.low + h.mid) / 2),
              highRing: above((h.mid + h.high) / 2),
              outside: above(Math.max(h.low, h.mid, h.high) + 4),
            } };
          }),
        };
      },
      { dpr, segment: SEGMENT, pattern: PATTERN },
    );

    const { colours, centre, segments } = result;
    const near = (a, b) => a.every((v, i) => Math.abs(v - b[i]) <= 8);
    const check = (name, ok, detail = '') => {
      checks++;
      if (!ok) failures.push(`dpr${dpr} ${theme}: ${name} ${detail}`);
    };
    const extent = (s, band) => ({ up: centre - segments[s].top, down: segments[s].bottom - centre, want: segments[s].half[band] });

    for (const [s, band] of [[0, 'low'], [1, 'mid'], [2, 'high']]) {
      const e = extent(s, band);
      check(`segment ${s} (${band} only) top height`, Math.abs(e.up - e.want) <= 1.5, `${e.up} vs ${e.want}`);
      check(`segment ${s} (${band} only) is symmetric`, Math.abs(e.down - e.up) <= 1.5, `${e.down} vs ${e.up}`);
    }
    check('segment 0 body is the low colour', near(segments[0].above.inLow, colours.low));
    check('segment 1 body is the mid colour', near(segments[1].above.inMid, colours.mid));
    check('segment 2 body is the high colour', near(segments[2].above.inHigh, colours.high));

    const s3 = segments[3];
    check('segment 3 heights stack low < mid < high', s3.half.low < s3.half.mid && s3.half.mid < s3.half.high);
    check('segment 3 low is in front', near(s3.above.inLow, colours.low));
    check('segment 3 mid ring is the mid colour', near(s3.above.midRing, colours.mid));
    check('segment 3 high ring is the high colour', near(s3.above.highRing, colours.high));
    check('segment 3 outside the tallest band is the ground', near(s3.above.outside, colours.bg));
    check('segment 3 top height', Math.abs(centre - s3.top - s3.half.high) <= 1.5, `${centre - s3.top} vs ${s3.half.high}`);

    await page.close();
  }
}

console.log(JSON.stringify({ checks, failures }, null, 2));
await browser.close();
process.exit(failures.length ? 1 : 0);
