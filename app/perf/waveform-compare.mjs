import { createRequire } from 'node:module';
import { readFileSync, mkdirSync, writeFileSync, readdirSync } from 'node:fs';

const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [root] = process.argv.slice(2);
mkdirSync(`${root}/compare`, { recursive: true });
const browser = await chromium.launch();
const page = await browser.newPage();
const views = ['list', 'detail', 'player-compact', 'player-expanded', 'player-expanded-zoomed', 'player-expanded-fit'];
let written = 0;

for (const variant of readdirSync(`${root}/before`)) {
  for (const view of views) {
    const before = readFileSync(`${root}/before/${variant}/${view}-crop.png`).toString('base64');
    const after = readFileSync(`${root}/after/${variant}/${view}-crop.png`).toString('base64');
    const png = await page.evaluate(async ([a, b, name]) => {
      const load = async (s) => {
        const img = new Image();
        img.src = `data:image/png;base64,${s}`;
        await img.decode();
        return img;
      };
      const A = await load(a);
      const B = await load(b);
      const pad = 16 * (A.naturalWidth > 1600 ? 2 : 1);
      const label = 28 * (A.naturalWidth > 1600 ? 2 : 1);
      const stacked = A.naturalWidth > 700;
      const width = stacked ? Math.max(A.naturalWidth, B.naturalWidth) + pad * 2 : A.naturalWidth + B.naturalWidth + pad * 3;
      const height = stacked
        ? A.naturalHeight + B.naturalHeight + label * 2 + pad * 3
        : Math.max(A.naturalHeight, B.naturalHeight) + label + pad * 2;
      const c = document.createElement('canvas');
      c.width = width;
      c.height = height;
      const ctx = c.getContext('2d');
      ctx.fillStyle = '#777';
      ctx.fillRect(0, 0, width, height);
      ctx.font = `${label * 0.6}px system-ui`;
      ctx.fillStyle = '#fff';
      if (stacked) {
        ctx.fillText(`before: ${name}`, pad, label * 0.75);
        ctx.drawImage(A, pad, label);
        const y = label + A.naturalHeight + pad;
        ctx.fillText('after', pad, y + label * 0.75);
        ctx.drawImage(B, pad, y + label);
      } else {
        ctx.fillText(`before: ${name}`, pad, label * 0.75);
        ctx.fillText('after', pad * 2 + A.naturalWidth, label * 0.75);
        ctx.drawImage(A, pad, label);
        ctx.drawImage(B, pad * 2 + A.naturalWidth, label);
      }
      return c.toDataURL('image/png').split(',')[1];
    }, [before, after, `${view} ${variant}`]);
    writeFileSync(`${root}/compare/${view}-${variant}.png`, Buffer.from(png, 'base64'));
    written++;
  }
}
console.log(`wrote ${written} comparison images`);
await browser.close();
