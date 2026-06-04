// Node-side Vite extension for the viewport dev server. Kept separate from
// viewport.config.js (which the browser imports) so this Node-only plugin
// never reaches the client bundle. The engine merges `plugins` into its Vite
// server (see viewport/bin/viewport.js → loadProjectVitePlugins).
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default {
  plugins: [svelte()],
};
