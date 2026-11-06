import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  // Cargo's build output is locked while it compiles; the Rust side has its own watcher.
  server: { port: 5173, strictPort: true, watch: { ignored: ['**/target/**', '**/src-tauri/**', '**/core/**'] } },
  // WebView2 is an evergreen Chromium.
  build: { target: 'chrome120' },
});
