import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// base: './'  -> assets use relative paths, so the bundle works when the Go
//                server embeds and serves it from any mount point.
// build.outDir 'dist' -> what server/ embeds via go:embed.
// server.proxy -> in `npm run dev`, forward /api to the Go backend on :8080
//                 so the React dev server gets live camera data with HMR.
export default defineConfig({
  plugins: [react()],
  base: './',
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
