import { cpSync, existsSync, mkdirSync, rmSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const repoRoot = dirname(dirname(fileURLToPath(import.meta.url)))
const publicRoot = join(repoRoot, 'public')

preparePdfJsAssets()
prepareTesseractAssets()

function preparePdfJsAssets() {
  const sourceRoot = join(repoRoot, 'node_modules', 'pdfjs-dist')
  const outputRoot = join(publicRoot, 'pdfjs')
  const directories = ['standard_fonts', 'wasm']

  requirePaths(directories.map((directory) => join(sourceRoot, directory)), 'PDF.js')
  rmSync(outputRoot, { recursive: true, force: true })
  for (const directory of directories) {
    cpSync(join(sourceRoot, directory), join(outputRoot, directory), { recursive: true })
  }
  console.log(`[pdfjs] prepared ${directories.join(' and ')} runtime assets`)
}

/** Copy only the browser worker, LSTM cores, and selected offline language. */
function prepareTesseractAssets() {
  const workerSource = join(repoRoot, 'node_modules', 'tesseract.js', 'dist', 'worker.min.js')
  const coreRoot = join(repoRoot, 'node_modules', 'tesseract.js-core')
  const languageSource = join(
    repoRoot,
    'node_modules',
    '@tesseract.js-data',
    'eng',
    '4.0.0_best_int',
    'eng.traineddata.gz',
  )
  const outputRoot = join(publicRoot, 'tesseract')
  const coreFiles = [
    'tesseract-core-lstm.wasm.js',
    'tesseract-core-lstm.wasm',
    'tesseract-core-simd-lstm.wasm.js',
    'tesseract-core-simd-lstm.wasm',
    'tesseract-core-relaxedsimd-lstm.wasm.js',
    'tesseract-core-relaxedsimd-lstm.wasm',
  ]

  requirePaths([
    workerSource,
    languageSource,
    ...coreFiles.map((file) => join(coreRoot, file)),
  ], 'Tesseract')
  rmSync(outputRoot, { recursive: true, force: true })
  mkdirSync(join(outputRoot, 'core'), { recursive: true })
  mkdirSync(join(outputRoot, 'lang'), { recursive: true })
  cpSync(workerSource, join(outputRoot, 'worker.min.js'))
  cpSync(languageSource, join(outputRoot, 'lang', 'eng.traineddata.gz'))
  for (const file of coreFiles) cpSync(join(coreRoot, file), join(outputRoot, 'core', file))
  console.log('[tesseract] prepared worker, LSTM cores, and English language data')
}

function requirePaths(paths, label) {
  for (const path of paths) {
    if (!existsSync(path)) {
      throw new Error(`Missing ${label} runtime asset: ${path}. Run npm install first.`)
    }
  }
}
