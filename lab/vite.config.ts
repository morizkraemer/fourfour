import fs from 'node:fs';
import { defineConfig } from 'vite';
import { labHub } from './node_modules/labkit/src/server';

export default defineConfig({
  root: import.meta.dirname,
  plugins: [labHub({ root: 'data' })],
  server: {
    allowedHosts: true,
    watch: null,
    fs: { allow: [import.meta.dirname, fs.realpathSync('node_modules/labkit')] },
  },
  preview: { allowedHosts: true },
  build: { outDir: 'dist', emptyOutDir: true, rollupOptions: { input: ['lab.html'] } },
});
