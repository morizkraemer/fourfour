import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { fileURLToPath, URL } from 'node:url';

// The shared design-system lives outside this app's root (single source for the
// canvas + the app). Alias `$ds` to its Svelte barrel and `$ds/*` to the folder;
// allow Vite's dev server to read outside root.
const designSystem = fileURLToPath(new URL('../prototyping/viewport/design-system', import.meta.url));

export default defineConfig({
  plugins: [svelte()],
  resolve: {
    alias: [
      { find: /^\$ds$/, replacement: `${designSystem}/svelte.js` },
      { find: /^\$ds\//, replacement: `${designSystem}/` },
    ],
  },
  server: {
    port: 5200,
    strictPort: false,
    fs: { allow: ['.', designSystem] },
  },
});
