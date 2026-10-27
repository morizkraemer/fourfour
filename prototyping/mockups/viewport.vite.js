import { svelte } from '@sveltejs/vite-plugin-svelte';

const allowParentFolder = {
  name: 'mockups-allow-parent-folder',
  config: () => ({ server: { fs: { allow: ['..'] } } }),
};

const allowAnyHost = {
  name: 'mockups-allow-any-host',
  config: () => ({ server: { allowedHosts: true } }),
};

export default {
  plugins: [svelte(), allowParentFolder, allowAnyHost],
};
