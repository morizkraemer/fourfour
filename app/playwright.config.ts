import { defineConfig, devices } from '@playwright/test';

const PORT = Number(process.env.FF_SMOKE_PORT ?? 5299);
const OUT_DIR = 'smoke/.dist';

export default defineConfig({
  testDir: 'smoke',
  testMatch: '*.spec.ts',
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 30_000,
  outputDir: 'smoke/.results',
  reporter: [['list']],
  use: {
    baseURL: `http://127.0.0.1:${PORT}`,
    ...devices['Desktop Chrome'],
    viewport: { width: 1600, height: 1000 },
    launchOptions: { args: ['--autoplay-policy=no-user-gesture-required'] },
  },
  webServer: {
    command: `npx vite build ${process.env.FF_PERF_PROFILE ? '--minify false ' : ''}--outDir ${OUT_DIR} --emptyOutDir --logLevel warn && npx vite preview --outDir ${OUT_DIR} --port ${PORT} --strictPort --host 127.0.0.1`,
    url: `http://127.0.0.1:${PORT}`,
    timeout: 180_000,
    reuseExistingServer: false,
  },
});
