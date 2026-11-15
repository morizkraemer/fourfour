import type { Environment } from 'vitest/runtime';

/**
 * Node globals with Vite's client transform, so `.svelte.ts` modules compile for the browser and `$state` builds real
 * proxies. The default `node` environment compiles them for the server, where `$state(x)` is `x` itself.
 */
export default {
  name: 'client-runtime',
  viteEnvironment: 'client',
  setup() {
    return { teardown() {} };
  },
} satisfies Environment;
