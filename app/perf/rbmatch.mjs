import { createRequire } from 'node:module';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';

/**
 * Renders one expanded player (overview + zoom) from raw PWV7 / PWV6 bytes through the real
 * WaveformDisplay, at an exact pixels-per-second and left edge, for the Rekordbox match.
 *
 *   PW_DIR=<dir with playwright> node perf/rbmatch.mjs <base url> <fixture.json> <rekordbox|ours> <out.png>
 *        [pxPerSec] [leftSec] [width] [zoomHeight] [overviewHeight] [overview=pwv6|detail]
 *
 * `fixture.json` is written by `waveform_compare track` (rekordbox and ours byte sets side by side).
 */
const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [base, fixturePath, side, out, pxPerSec, leftSec, width, zoomHeight, overviewHeight, overviewFrom = 'detail'] = process.argv.slice(2);
const fixture = JSON.parse(readFileSync(fixturePath, 'utf8'));
const bytes = fixture[side];
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: Math.ceil(+width) + 20, height: 800 }, deviceScaleFactor: Number(process.env.DPR || 1) });
page.on('pageerror', (e) => console.log('pageerror', e.message));
await page.goto(`${base}/rbmatch.html`, { waitUntil: 'load' });
await page.waitForFunction(() => window.rbShow);
await page.evaluate(
  (o) => window.rbShow(o),
  {
    pwv7: Array.from(bytes.pwv7),
    pwv6: Array.from(bytes.pwv6),
    durationMs: fixture.duration_secs * 1000,
    leftSec: +leftSec,
    pxPerSec: +pxPerSec,
    width: +width,
    overviewHeight: +overviewHeight,
    zoomHeight: +zoomHeight,
    overviewFromPwv6: overviewFrom === 'pwv6',
  },
);
await page.waitForTimeout(400);
mkdirSync(out.replace(/\/[^/]*$/, ''), { recursive: true });
await page.locator('#stage').screenshot({ path: out });
await browser.close();
