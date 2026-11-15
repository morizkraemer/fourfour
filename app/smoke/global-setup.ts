import { chromium, webkit, type FullConfig } from '@playwright/test';
import { loadavg } from 'node:os';
import { loadFactor } from './load-factor.ts';

export default async function globalSetup(config: FullConfig) {
  const { baseURL } = config.projects[0].use;
  const browserType = process.env.FF_SMOKE_BROWSER === 'webkit' ? webkit : chromium;
  const browser = await browserType.launch();
  const page = await browser.newPage();
  await page.goto(baseURL!, { waitUntil: 'load', timeout: 120_000 });
  await browser.close();
  console.log(`smoke: load average ${loadavg()[0].toFixed(1)}, timeout factor x${loadFactor()}`);
}
