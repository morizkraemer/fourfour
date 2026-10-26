import { svelte } from '@sveltejs/vite-plugin-svelte';

const allowParentFolder = {
  name: 'mockups-allow-parent-folder',
  config: () => ({ server: { fs: { allow: ['..'] } } }),
};

export default {
  plugins: [svelte(), allowParentFolder],
};
