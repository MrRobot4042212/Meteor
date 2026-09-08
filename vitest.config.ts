import { defineConfig } from 'vitest/config';
import path from 'node:path';

export default defineConfig({
  test: {
    // Pure logic only: no jsdom, no Tauri runtime. Anything that needs a webview
    // or the native side is verified by `cargo test` or by running the app.
    environment: 'node',
    include: ['src/**/*.test.ts'],
  },
  resolve: {
    alias: { '@': path.resolve(__dirname, 'src') },
  },
});
