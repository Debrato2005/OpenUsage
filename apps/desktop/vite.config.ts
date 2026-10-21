import { defineConfig } from 'vite';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  root: 'apps/desktop',
  plugins: [tailwindcss()],
  clearScreen: false,
  server: { host: '127.0.0.1', port: 1420, strictPort: true, watch: { ignored: ['**/src-tauri/**'] } },
  preview: { host: '127.0.0.1', port: 1420, strictPort: true },
  build: { target: 'es2022', sourcemap: false },
});
