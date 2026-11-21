import path from 'node:path'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'
import { dependenciesFromHere, repo, sdk, tessera, web } from './sdk.config.ts'

// The kit package is served from wherever a host puts it, so every asset path is relative.
export default defineConfig({
  base: './',
  plugins: [dependenciesFromHere(), react()],
  resolve: {
    // @kition/sdk (its kdm replica and the editor's session) reads replicas with loro-crdt, whose
    // browser entry loads its WebAssembly itself; the entry Vite's server would pick imports it as
    // a module, which needs a plugin. Tests run in Node, on loro-crdt's Node entry.
    alias: process.env.VITEST ? [] : [{ find: /^loro-crdt$/, replacement: 'loro-crdt/browser' }],
    // One React for the page.
    dedupe: ['react', 'react-dom'],
  },
  // loro-crdt stays unbundled, so its WebAssembly is found next to it.
  optimizeDeps: { exclude: ['loro-crdt'] },
  worker: { format: 'es', plugins: () => [dependenciesFromHere()] },
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
