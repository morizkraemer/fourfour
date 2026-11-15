import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  define: { __FF_BUILD__: JSON.stringify({ number: 'test', commit: '' }) },
  test: {
    environment: './src/stores/client-runtime.env.ts',
    include: ['src/**/*.test.ts'],
    testTimeout: 30_000,
    hookTimeout: 30_000,
  },
});
