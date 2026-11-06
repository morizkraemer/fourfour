import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { createServer, type Server } from 'node:http';
import type { AddressInfo } from 'node:net';
import { test, type Browser, type Page } from '@playwright/test';
import { installTauriMock } from './tauri-mock.ts';

const PAYLOAD = process.env.FF_PERF_FIRST_SCREEN;
const OUT = process.env.FF_PERF_FIRST_SCREEN_OUT ?? 'first-screen';
const BOOT_ROWS = !!process.env.FF_PERF_FIRST_SCREEN_BOOT;

type Mode = 'skeleton' | 'boot' | 'row' | 'late';

function installAnswers({ payloadUrl, mode }: { payloadUrl: string; mode: Mode }) {
  const w = window as unknown as {
    __TAURI_INTERNALS__: { invoke: (cmd: string, args: Record<string, unknown>) => Promise<unknown> };
  };
  const invoke = w.__TAURI_INTERNALS__.invoke;
  w.__TAURI_INTERNALS__.invoke = async (cmd, args = {}) => {
    if (cmd === 'load_state') {
      if (mode === 'skeleton' || mode === 'boot') return new Promise(() => {});
      return (await fetch(payloadUrl)).json();
    }
    if (cmd === 'get_row_previews' && mode === 'row') return new Promise(() => {});
    return invoke(cmd, args);
  };
}

function serve(body: Buffer): Promise<{ server: Server; url: string }> {
  const server = createServer((_, res) => {
    res.writeHead(200, { 'content-type': 'application/json', 'access-control-allow-origin': '*', 'cache-control': 'no-store' });
    res.end(body);
  });
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve({ server, url: `http://127.0.0.1:${(server.address() as AddressInfo).port}/state.json` })));
}

function normalise(html: string) {
  return html.replace(/<!--[\s\S]*?-->/g, '').replace(/\ssvelte-[a-z0-9]+/g, '').replace(/svelte-[a-z0-9]+\s?/g, '');
}

async function freezeAnimations(page: Page) {
  await page.evaluate(() => {
    for (const animation of document.getAnimations()) {
      animation.pause();
      animation.currentTime = 700;
    }
  });
}

async function capture(page: Page, name: string, lines: string[]) {
  await page.evaluate(() => document.fonts.ready);
  await freezeAnimations(page);
  const html = normalise(await page.evaluate(() => document.body.outerHTML));
  const shot = await page.screenshot({ animations: 'allow' });
  writeFileSync(`${OUT}-${name}.html`, html);
  writeFileSync(`${OUT}-${name}.png`, shot);
  const sha = (data: string | Buffer) => createHash('sha256').update(data).digest('hex').slice(0, 12);
  lines.push(`${name.padEnd(9)} png ${sha(shot)} ${String(shot.length).padStart(7)} B   html ${sha(html)} ${html.split('\n').length} lines`);
}

async function newPage(browser: Browser, url: string, mode: Mode, storageState?: Awaited<ReturnType<Page['context']>['storageState']>) {
  const context = await browser.newContext({ viewport: { width: 1600, height: 1000 }, storageState });
  await context.addInitScript(installTauriMock, { tracks: [], playlists: [] });
  await context.addInitScript(installAnswers, { payloadUrl: url, mode });
  return { context, page: await context.newPage() };
}

test.describe('first screen identity', () => {
  test.skip(!PAYLOAD, 'set FF_PERF_FIRST_SCREEN=/path/state.json (a load_state payload)');
  test.setTimeout(300_000);

  test('skeleton, boot rows (a payload of 20,000 tracks or more), first rows and the settled screen', async ({ browser }) => {
    const { server, url } = await serve(readFileSync(PAYLOAD!));
    const lines: string[] = [];

    const first = await newPage(browser, url, 'skeleton');
    await first.page.goto('/');
    await first.page.locator('[data-table-skeleton]').waitFor();
    await first.page.waitForTimeout(1500);
    await capture(first.page, 'skeleton', lines);
    await first.context.close();

    const seed = await newPage(browser, url, 'late');
    await seed.page.goto('/');
    await seed.page.locator('[data-track-row]').first().waitFor({ timeout: 120_000 });
    await seed.page.waitForTimeout(2500);
    const saved = await seed.context.storageState();
    await seed.context.close();

    if (BOOT_ROWS) {
      const boot = await newPage(browser, url, 'boot', saved);
      await boot.page.goto('/');
      await boot.page.locator('[data-boot-row]').first().waitFor({ timeout: 120_000 });
      await boot.page.waitForTimeout(1500);
      await capture(boot.page, 'boot', lines);
      await boot.context.close();
    }

    const row = await newPage(browser, url, 'row', saved);
    await row.page.goto('/');
    await row.page.locator('[data-track-row]').first().waitFor({ timeout: 120_000 });
    await row.page.waitForTimeout(1500);
    await capture(row.page, 'row', lines);
    await row.context.close();

    const late = await newPage(browser, url, 'late', saved);
    await late.page.goto('/');
    await late.page.locator('[data-track-row]').first().waitFor({ timeout: 120_000 });
    await late.page.waitForTimeout(4000);
    await capture(late.page, 'late', lines);
    await late.context.close();

    server.close();
    console.log(lines.join('\n'));
    writeFileSync(`${OUT}-summary.txt`, `${lines.join('\n')}\n`);
  });
});
