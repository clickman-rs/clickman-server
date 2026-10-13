import { defineConfig } from 'vite';
import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { fileURLToPath } from 'node:url';

export default defineConfig(({ command }) => ({
  plugins: [tailwindcss(), sveltekit()],
  // Development follows library source; production consumes its packaged output.
  resolve: {
    alias:
      command === 'serve'
        ? [
            {
              find: '@clickman/dashboard/styles.css',
              replacement: fileURLToPath(
                new URL('../../packages/dashboard/styles/input.css', import.meta.url),
              ),
            },
            {
              find: /^@clickman\/dashboard$/,
              replacement: fileURLToPath(
                new URL('../../packages/dashboard/src/lib/index.ts', import.meta.url),
              ),
            },
          ]
        : [],
  },
  server: {
    proxy: {
      '/graphql': {
        target: process.env.CLICKMAN_PROXY_TARGET ?? 'http://127.0.0.1:4132',
        changeOrigin: true,
      },
    },
  },
}));
