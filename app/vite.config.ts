import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { fileURLToPath, URL } from 'node:url';
import { execSync } from 'node:child_process';

// The shared design-system lives outside this app's root (single source for the
// canvas + the app). Alias `$ds` to its Svelte barrel and `$ds/*` to the folder;
// allow Vite's dev server to read outside root.
const designSystem = fileURLToPath(new URL('../prototyping/viewport/design-system', import.meta.url));
const host = process.env.TAURI_DEV_HOST;

function gitBuild(): { number: string; commit: string } {
  try {
    const git = (args: string) => execSync(`git ${args}`, { stdio: ['ignore', 'pipe', 'ignore'] }).toString().trim();
    return { number: git('rev-list --count HEAD'), commit: git('rev-parse --short HEAD') };
  } catch {
    return { number: 'dev', commit: '' };
  }
}

export default defineConfig({
  plugins: [svelte()],
  define: { __FF_BUILD__: JSON.stringify(gitBuild()) },
  clearScreen: false,
  resolve: {
    alias: [
      { find: /^\$ds$/, replacement: `${designSystem}/svelte.js` },
      { find: /^\$ds\//, replacement: `${designSystem}/` },
    ],
  },
  build: {
    target: 'safari16',
    cssCodeSplit: false,
    modulePreload: { resolveDependencies: () => [] },
    rollupOptions: {
      treeshake: { moduleSideEffects: (id) => !id.startsWith(`${designSystem}/components/`) || !id.endsWith('.svelte') },
    },
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
