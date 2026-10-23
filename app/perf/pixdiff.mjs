import { createRequire } from 'node:module';
import { readFileSync, readdirSync } from 'node:fs';

const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [dirA, dirB, threshold = '0'] = process.argv.slice(2);
const browser = await chromium.launch();
const page = await browser.newPage();
const names = readdirSync(dirA).filter((f) => f.endsWith('.png')).sort();
let worst = 0;
let failed = false;
for (const name of names) {
  const a = readFileSync(`${dirA}/${name}`).toString('base64');
  const b = readFileSync(`${dirB}/${name}`).toString('base64');
  const r = await page.evaluate(async ([a, b]) => {
    const load = async (s) => {
      const img = new Image();
      img.src = `data:image/png;base64,${s}`;
      await img.decode();
      const c = new OffscreenCanvas(img.naturalWidth, img.naturalHeight);
      const ctx = c.getContext('2d');
      ctx.drawImage(img, 0, 0);
      return ctx.getImageData(0, 0, c.width, c.height);
    };
    const A = await load(a);
    const B = await load(b);
    if (A.width !== B.width || A.height !== B.height) return { size: 'mismatch' };
    let max = 0, diffPixels = 0, minX = 1e9, minY = 1e9, maxX = -1, maxY = -1;
    for (let i = 0; i < A.data.length; i += 4) {
      const d = Math.max(
        Math.abs(A.data[i] - B.data[i]),
        Math.abs(A.data[i + 1] - B.data[i + 1]),
        Math.abs(A.data[i + 2] - B.data[i + 2]),
        Math.abs(A.data[i + 3] - B.data[i + 3]),
      );
      if (d > 0) {
        diffPixels++;
        const p = i / 4;
        const x = p % A.width, y = Math.floor(p / A.width);
        minX = Math.min(minX, x); maxX = Math.max(maxX, x); minY = Math.min(minY, y); maxY = Math.max(maxY, y);
      }
      if (d > max) max = d;
    }
    return { width: A.width, height: A.height, max, diffPixels, bbox: diffPixels ? [minX, minY, maxX, maxY] : null };
  }, [a, b]);
  console.log(name.padEnd(26), JSON.stringify(r));
  if (r.size || r.max > Number(threshold)) failed = true;
  worst = Math.max(worst, r.max ?? 255);
}
console.log(`MAX channel difference over ${names.length} screenshots: ${worst} (threshold ${threshold})`);
await browser.close();
process.exit(failed ? 1 : 0);
