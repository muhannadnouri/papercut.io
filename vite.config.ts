import {
  configDefaults,
  defineConfig,
} from 'vitest/config'
import react from '@vitejs/plugin-react'
import {
  relative,
  resolve,
  sep,
} from 'path'

function isRootHtmlFixture(path: string): boolean {
  const relativePath = relative(__dirname, path)
  return relativePath !== 'index.html' &&
    relativePath.endsWith('.html') &&
    !relativePath.startsWith('..') &&
    !relativePath.includes(sep)
}

const sourcePdf = resolve(__dirname, '.cache/android-production/pdf-worker-work/build/generic-legacy')

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: process.env.PAPERCUT_PDF_OCR_FROM_SOURCE === '1' ? {
      'pdfjs-dist/legacy/build/pdf.mjs': resolve(sourcePdf, 'build/pdf.mjs'),
      'pdfjs-dist/legacy/build/pdf.worker.min.mjs?url': resolve(sourcePdf, 'build/pdf.worker.min.mjs') + '?url',
      'pdfjs-dist/legacy/web/pdf_viewer.mjs': resolve(sourcePdf, 'web/pdf_viewer.mjs'),
    } : {},
  },
  test: {
    // Source-build checkouts contain upstream tests that are not part of Papercut's suite.
    exclude: [...configDefaults.exclude, '**/.cache/**'],
  },
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
