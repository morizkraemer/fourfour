import { defineConfig } from 'vite';
import { labApi } from './server/plugin';

export default defineConfig({
  root: import.meta.dirname,
  plugins: [labApi(import.meta.dirname)],
  server: { allowedHosts: true, watch: null },
  preview: { allowedHosts: true },
  build: { outDir: 'dist', emptyOutDir: true },
  test: { include: ['src/**/*.test.ts', 'server/**/*.test.ts'] },
});
