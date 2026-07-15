import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { relative, resolve, sep } from 'path'

function isRootHtmlFixture(path: string): boolean {
  const relativePath = relative(__dirname, path)
  return relativePath !== 'index.html' &&
    relativePath.endsWith('.html') &&
    !relativePath.startsWith('..') &&
    !relativePath.includes(sep)
}

export default defineConfig({
  plugins: [react()],
  server: {
    watch: {
      ignored: [
        '**/.cache/**',
        '**/.venv-silma/**',
        '**/public/documents/**',
        '**/sidecars/**/__pycache__/**',
        '**/sidecars/**/runtime/**',
        '**/src-tauri/target/**',
        '**/src-tauri/gen/**',
        '**/src-tauri/tts/runtime/**',
        '**/*.epub',
        '**/*.zip',
        isRootHtmlFixture,
      ],
    },
  },
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, 'index.html'),
      },
    },
  },
  publicDir: 'public',
})
