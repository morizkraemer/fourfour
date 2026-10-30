import { spawn, spawnSync, type ChildProcess } from 'node:child_process';
import { createRequire } from 'node:module';
import { existsSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import type { Browser, BrowserContext, Page } from '@playwright/test';
import type { BootOptions, Scenario, ScenarioContext, ShotOptions } from '../scenarios/types.ts';
import { flagValue } from './flags.ts';

const usage =
  'usage: npm run hub:shot -- --app <worktree or app dir> --scenarios scenarios/<file>.ts [--id <card-id>] [--only a,b] [--tracks N] [--port 5399]';
const argv = process.argv;
const appArg = flagValue(argv, 'app');
const scenariosArg = flagValue(argv, 'scenarios');
const id = flagValue(argv, 'id');
if (!appArg || !scenariosArg) throw new Error(usage);

const labRoot = resolve(import.meta.dirname, '..');
const appDir = existsSync(join(resolve(appArg), 'smoke')) ? resolve(appArg) : join(resolve(appArg), 'app');
if (!existsSync(join(appDir, 'smoke', 'tauri-mock.ts'))) throw new Error(`no smoke/tauri-mock.ts under ${appArg}; the branch needs the smoke harness`);
const only = flagValue(argv, 'only')?.split(',');
const port = Number(flagValue(argv, 'port') ?? 5399);
const hasFfmpeg = spawnSync('ffmpeg', ['-version']).status === 0;

const tracks = flagValue(argv, 'tracks');
if (tracks) process.env.FF_PERF_TRACKS = tracks;

const requireFromApp = createRequire(join(appDir, 'package.json'));
const viteBin = join(dirname(requireFromApp.resolve('vite/package.json')), 'bin', 'vite.js');
const { chromium } = requireFromApp('@playwright/test') as typeof import('@playwright/test');

function run(command: string, args: string[], cwd: string) {
  const result = spawnSync(command, args, { cwd, stdio: ['ignore', 'inherit', 'inherit'] });
  if (result.status !== 0) throw new Error(`${command} ${args.join(' ')} failed in ${cwd}`);
}

async function waitForServer(url: string, server: ChildProcess) {
  for (let i = 0; i < 120; i++) {
    if (server.exitCode !== null) throw new Error('preview server exited early');
    try {
      if ((await fetch(url)).ok) return;
    } catch {
      await new Promise((done) => setTimeout(done, 250));
    }
  }
  throw new Error(`preview server did not answer at ${url}`);
}

function convertVideo(webm: string, base: string, withGif: boolean, cwd: string) {
  if (!hasFfmpeg) {
    console.warn(`no ffmpeg: keeping ${webm}`);
    return;
  }
  run('ffmpeg', ['-y', '-loglevel', 'error', '-i', webm, '-an', '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-movflags', '+faststart', '-vf', 'scale=trunc(iw/2)*2:trunc(ih/2)*2', `${base}.mp4`], cwd);
  if (withGif) {
    run('ffmpeg', ['-y', '-loglevel', 'error', '-i', webm, '-vf', 'fps=12,scale=720:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse', `${base}.gif`], cwd);
  }
}

async function main() {
  const scenarios = ((await import(pathToFileURL(resolve(labRoot, scenariosArg!)).href)) as { default: Record<string, Scenario> }).default;
  const smoke = join(appDir, 'smoke');
  const { installTauriMock } = await import(pathToFileURL(join(smoke, 'tauri-mock.ts')).href);
  const { fixtureAnalysis, fixtureState } = await import(pathToFileURL(join(smoke, 'fixture.ts')).href);

  const buildDir = mkdtempSync(join(tmpdir(), 'review-shot-'));
  process.on('exit', () => rmSync(buildDir, { recursive: true, force: true }));
  run(process.execPath, [viteBin, 'build', '--outDir', buildDir, '--emptyOutDir', '--logLevel', 'error'], appDir);
  const server = spawn(process.execPath, [viteBin, 'preview', '--outDir', buildDir, '--port', String(port), '--strictPort', '--host', '127.0.0.1'], {
    cwd: appDir,
    stdio: 'ignore',
  });
  process.on('exit', () => server.kill());
  const base = `http://127.0.0.1:${port}`;
  await waitForServer(base, server);

  const browser: Browser = await chromium.launch({ args: ['--autoplay-policy=no-user-gesture-required'] });
  const failures: string[] = [];
  const cardMediaDirs = new Set<string>();

  for (const [name, scenario] of Object.entries(scenarios)) {
    if (only && !only.includes(name)) continue;
    const outDir = join(labRoot, 'data', 'cards', id ?? name, 'media');
    mkdirSync(outDir, { recursive: true });
    cardMediaDirs.add(outDir);
    let context: BrowserContext | null = null;
    let page: Page | null = null;
    let recording = false;
    const warnings: string[] = [];

    const closeContext = async () => {
      if (context) await context.close();
      context = null;
      page = null;
    };

    const ctx: ScenarioContext = {
      async boot(options: BootOptions = {}) {
        await closeContext();
        const viewport = options.viewport ?? { width: 1440, height: 900 };
        recording = options.video ?? false;
        context = await browser.newContext({
          viewport,
          ...(recording ? { recordVideo: { dir: join(outDir, '.video'), size: viewport } } : {}),
        });
        const p = await context.newPage();
        page = p;
        p.on('pageerror', (error) => warnings.push(`pageerror: ${error.message}`));
        p.on('console', (message) => {
          if (message.type() === 'error') warnings.push(`console.error: ${message.text()}`);
        });
        await p.exposeFunction('__smokeAnalysis', (trackId: number) => fixtureAnalysis(trackId));
        const state = options.state ? options.state(fixtureState()) : fixtureState();
        await p.addInitScript(installTauriMock, state);
        if (options.holdLoad) await p.addInitScript(() => ((window as unknown as { __smokeHoldLoad: boolean }).__smokeHoldLoad = true));
        if (options.storage) {
          await p.addInitScript((entries: Record<string, string>) => {
            for (const [key, value] of Object.entries(entries)) localStorage.setItem(key, value);
          }, options.storage);
        }
        await p.goto(base);
        if (!options.holdLoad) await p.locator('[data-track-row]').first().waitFor();
        return p;
      },
      async shot(label: string, options: ShotOptions = {}) {
        if (!page) throw new Error('boot before shot');
        await page.screenshot({ path: join(outDir, `${label}.png`), ...(options.clip ? { clip: options.clip } : {}) });
        console.log(`png ${label}.png`);
      },
      async finishVideo(label: string, options: { gif?: boolean } = {}) {
        if (!page || !recording) throw new Error('finishVideo needs boot({ video: true })');
        const handle = page.video()!;
        await closeContext();
        convertVideo(await handle.path(), join(outDir, label), options.gif ?? true, outDir);
        console.log(`video ${label}.mp4`);
      },
      pageOrThrow() {
        if (!page) throw new Error('boot first');
        return page;
      },
      emit(event: string, payload: unknown) {
        if (!page) throw new Error('boot first');
        return page.evaluate(
          ([e, p]) => (window as unknown as { __smokeEmit: (e: string, p: unknown) => void }).__smokeEmit(e as string, p),
          [event, payload] as const,
        );
      },
    };

    console.log(`scenario ${name}`);
    try {
      await scenario(ctx);
    } catch (error) {
      failures.push(`${name}: ${(error as Error).message}`);
    }
    await closeContext();
    for (const w of warnings) console.warn(`  ${name}: ${w}`);
  }

  await browser.close();
  server.kill();
  for (const dir of cardMediaDirs) rmSync(join(dir, '.video'), { recursive: true, force: true });
  console.log(`captured into data/cards/<card id>/media`);
  if (failures.length) {
    console.error(failures.join('\n'));
    process.exit(1);
  }
  process.exit(0);
}

main();
