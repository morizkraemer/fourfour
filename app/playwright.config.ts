import { defineConfig, devices } from '@playwright/test';
import { loadFactor } from './smoke/load-factor.ts';

const PORT = Number(process.env.FF_SMOKE_PORT ?? 5299);
const OUT_DIR = 'smoke/.dist';
const WEBKIT = process.env.FF_SMOKE_BROWSER === 'webkit';
const FACTOR = loadFactor();

export default defineConfig({
  testDir: 'smoke',
  testMatch: '*.spec.ts',
  fullyParallel: false,
  workers: 1,
  retries: Number(process.env.FF_SMOKE_RETRIES ?? 0),
  timeout: 30_000 * FACTOR,
  expect: { timeout: 5_000 * FACTOR },
  globalSetup: './smoke/global-setup.ts',
  outputDir: 'smoke/.results',
  reporter: [['list'], ['./smoke/reporter.ts']],
  use: {
    baseURL: `http://127.0.0.1:${PORT}`,
    ...(WEBKIT ? devices['Desktop Safari'] : devices['Desktop Chrome']),
    navigationTimeout: 30_000 * FACTOR,
    actionTimeout: 10_000 * FACTOR,
    viewport: { width: 1600, height: 1000 },
    launchOptions: WEBKIT ? {} : { args: ['--autoplay-policy=no-user-gesture-required'] },
  },
  webServer: {
    command: `npx vite build ${process.env.FF_PERF_PROFILE ? '--minify false ' : ''}--outDir ${OUT_DIR} --emptyOutDir --logLevel warn && npx vite preview --outDir ${OUT_DIR} --port ${PORT} --strictPort --host 127.0.0.1`,
    url: `http://127.0.0.1:${PORT}`,
    timeout: 180_000,
    reuseExistingServer: false,
  },
});
