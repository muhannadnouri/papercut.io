import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { extname, relative, resolve, sep } from 'path'

const ignoredWatchDirs = [
  '/.cache/',
  '/.venv-silma/',
  '/sidecars/',
  '/src-tauri/target/',
  '/src-tauri/tts/runtime/',
]

const ignoredRootWatchExtensions = new Set(['.epub', '.zip'])

function isIgnoredWatchPath(path: string) {
  const normalized = path.split(sep).join('/')
  if (ignoredWatchDirs.some((dir) => normalized.includes(dir))) return true

  const relativePath = relative(__dirname, path)
  if (relativePath.includes(sep) || relativePath.startsWith('..')) return false
  if (relativePath !== 'index.html' && extname(relativePath) === '.html') return true
  return ignoredRootWatchExtensions.has(extname(relativePath))
}

export default defineConfig({
  plugins: [react()],
  server: {
    watch: {
      ignored: isIgnoredWatchPath,
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
