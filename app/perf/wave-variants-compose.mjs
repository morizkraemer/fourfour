import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';

const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [dir] = process.argv.slice(2);
const variants = ['a', 'e', 'f', 'g'];
const dataUrl = (name) => `data:image/png;base64,${readFileSync(`${dir}/${name}.png`).toString('base64')}`;

const listCell = (v) => `<div class="cell"><div class="tag">${v}</div><div class="crop"><img src="${dataUrl(`${v}-list`)}"></div></div>`;
const detailCell = (v) => `<div class="cell"><div class="tag">${v}</div><img src="${dataUrl(`${v}-detail`)}"></div>`;
const html = `<body style="margin:0;background:#222;color:#fff;font:20px monospace">
<style>.row{display:flex;gap:16px;margin:8px}.cell{position:relative}.tag{position:absolute;left:4px;top:4px;background:#000a;padding:2px 8px}
.crop{width:300px;height:1040px;overflow:hidden}.crop img{margin-left:-180px;margin-top:-50px;width:1756px}</style>
<div class="row">${variants.map(listCell).join('')}</div>
<div class="row">${variants.map(detailCell).join('')}</div></body>`;

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1400, height: 800 } });
await page.setContent(html);
await page.screenshot({ path: `${dir}/compare-all.png`, fullPage: true });
await browser.close();
