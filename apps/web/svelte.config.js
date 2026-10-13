import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import adapter from '@sveltejs/adapter-static';

const base = process.env.CLICKMAN_BASE_PATH ?? '';
if (
  base &&
  (!base.startsWith('/') || base.endsWith('/') || base.includes('?') || base.includes('#'))
)
  throw new Error('CLICKMAN_BASE_PATH must be empty or an absolute path without a trailing slash.');

export default {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({ pages: 'dist', assets: 'dist', fallback: 'index.html' }),
    paths: { base, relative: false },
  },
};
