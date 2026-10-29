import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { fileURLToPath, URL } from 'node:url';

export default defineConfig({
  plugins: [svelte()],
  resolve: {
    alias: {
      'vitest-environment-client-runtime': fileURLToPath(new URL('./src/stores/client-runtime.env.ts', import.meta.url)),
    },
  },
  test: {
    environment: 'client-runtime',
    include: ['src/**/*.test.ts'],
  },
});
