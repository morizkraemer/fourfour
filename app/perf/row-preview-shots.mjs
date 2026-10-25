import { createRequire } from 'node:module';
import { mkdirSync } from 'node:fs';

const require = createRequire(`${process.env.PW_DIR}/`);
const { chromium } = require('playwright');

const [base, outDir, dsf = '2', count = '200'] = process.argv.slice(2);
mkdirSync(outDir, { recursive: true });

const LIGHT_BASE_TOKENS = `:root[data-theme='light']{
  --ff-bg:#f4f5f7;--ff-surface:#ffffff;--ff-elev:#eceef1;--ff-elev-hi:#e3e6ea;
  --ff-text:#16181d;--ff-text-mid:#4b515c;--ff-muted:#7b8290;--ff-faint:#a6adb8;
  --ff-border:rgba(0,0,0,.08);--ff-border-hi:rgba(0,0,0,.14);--ff-hover:rgba(0,0,0,.04);
  --ff-select:rgba(0,0,0,.07);--ff-accent:#111111;--ff-accent-hi:#000000;color-scheme:light}
  :root[data-theme='light'] body{background:var(--ff-bg);color:var(--ff-text)}`;

const browser = await chromium.launch();
const errors = [];

for (const theme of ['dark', 'light']) {
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: Number(dsf) });
  if (theme === 'light') {
    await page.addInitScript((css) => {
      const apply = () => {
        if (!document.documentElement || !document.head) return false;
        document.documentElement.dataset.theme = 'light';
        const style = document.createElement('style');
        style.textContent = css;
        document.head.append(style);
        return true;
      };
      if (!apply()) {
        const observer = new MutationObserver(() => apply() && observer.disconnect());
        observer.observe(document, { childList: true, subtree: true });
      }
    }, LIGHT_BASE_TOKENS);
  }
  page.on('console', (m) => {
    if (m.type() === 'error') errors.push(`console: ${m.text()}`);
  });
  page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));
  await page.goto(`${base}?mock=${count}`, { waitUntil: 'commit', timeout: 0 });
  await page.waitForSelector('[data-track-row]', { timeout: 120000 });
  await page.addStyleTag({ content: '*,*::before,*::after{animation:none!important;transition:none!important;caret-color:transparent!important}' });
  await page.waitForTimeout(1500);
  await page.locator('[data-track-row]').nth(2).click();
  await page.waitForTimeout(800);
  for (const [name, selector] of [['list', '.ff-table__scroll'], ['detail', '.ff-detail-pane']]) {
    const box = await page.locator(selector).first().boundingBox();
    await page.screenshot({
      path: `${outDir}/${theme}-${name}.png`,
      clip: { x: Math.floor(box.x), y: Math.floor(box.y), width: Math.ceil(box.width), height: Math.ceil(box.height) },
    });
  }
  await page.close();
}

console.log(errors.length ? errors.join('\n') : 'no console errors');
await browser.close();
