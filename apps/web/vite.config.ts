import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  base: './',
  plugins: [tailwindcss(), svelte()],
  server: { proxy: { '/graphql': { target: process.env.CLICKMAN_PROXY_TARGET ?? 'http://127.0.0.1:4132', changeOrigin: true } } },
});
