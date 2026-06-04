import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { fileURLToPath, URL } from 'node:url';

// The shared design-system lives outside this app's root (single source for the
// canvas + the app). Alias `$ds` to its Svelte barrel and `$ds/*` to the folder;
// allow Vite's dev server to read outside root.
const designSystem = fileURLToPath(new URL('../prototyping/viewport/design-system', import.meta.url));
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  resolve: {
    alias: [
      { find: /^\$ds$/, replacement: `${designSystem}/svelte.js` },
      { find: /^\$ds\//, replacement: `${designSystem}/` },
    ],
  },
  server: {
    port: 5200,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: 'ws', host, port: 5201 }
      : undefined,
    fs: { allow: ['.', designSystem] },
  },
});
