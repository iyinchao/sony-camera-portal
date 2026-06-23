import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { fileURLToPath } from 'node:url'

// base: './'  -> assets use relative paths, so the bundle works when the Rust
//                server embeds and serves it from any mount point.
// build.outDir 'dist' -> what packages/cli embeds via rust-embed.
// server.proxy -> in `npm run dev`, forward /api to the backend on :8080 (HMR).
export default defineConfig({
  plugins: [react(), tailwindcss()],
  base: './',
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
  server: {
    port: 5173,
    proxy: {
      '/api': 'http://127.0.0.1:8080',
    },
  },
})
