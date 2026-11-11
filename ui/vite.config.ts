import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  base: './',
  clearScreen: false,
  resolve: { preserveSymlinks: true },
  server: {
    port: 5173,
    strictPort: true,
  },
  build: { target: 'es2022', sourcemap: false },
});
