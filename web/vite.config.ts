import path from 'node:path'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'
import { dependenciesFromHere, repo, sdk, tessera, web } from './sdk.config.ts'

// The kit package is served from wherever a host puts it, so every asset path is relative.
export default defineConfig({
  base: './',
  plugins: [dependenciesFromHere(), react()],
  resolve: {
    // One React for the page.
    dedupe: ['react', 'react-dom'],
  },
  worker: { format: 'es' },
  server: {
    port: 5190,
    // The repository (the engine's schema file), and the kition-sdk and Tessera checkouts KITION_LINK=1 links.
    fs: { allow: [repo, sdk, tessera] },
  },
  test: { include: ['src/**/*.spec.ts'], environment: 'node' },
  build: {
    outDir: 'dist',
    chunkSizeWarningLimit: 4096,
    rollupOptions: { input: { index: path.join(web, 'index.html'), dev: path.join(web, 'dev.html') } },
  },
})
