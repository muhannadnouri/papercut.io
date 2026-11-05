import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import {
  cpSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
} from 'node:fs'
import { join } from 'node:path'
import { ROOT as repoRoot } from '../lib/paths.js'

const publicRoot = join(repoRoot, 'public')
const npmPdfRoot = join(repoRoot, 'node_modules/pdfjs-dist')
const fromSource = process.env.PAPERCUT_PDF_OCR_FROM_SOURCE === '1'
const cache = join(repoRoot, '.cache/android-production')
const pdfBuild = join(cache, 'pdf-worker-work/build/generic-legacy')
const wasmRoot = fromSource ? join(pdfBuild, 'web/wasm') : join(npmPdfRoot, 'wasm')
const coreRoot = fromSource ? join(cache, 'source-ocr') : join(repoRoot, 'node_modules/tesseract.js-core')
const coreFiles = [
  'tesseract-core-lstm.wasm.js',
  'tesseract-core-lstm.wasm',
  'tesseract-core-simd-lstm.wasm.js',
  'tesseract-core-simd-lstm.wasm',
  'tesseract-core-relaxedsimd-lstm.wasm.js',
  'tesseract-core-relaxedsimd-lstm.wasm',
]

if (fromSource) {
  const pdfReceipt = readSourceReceipt(pdfBuild, 'build-source-pdf-codecs.js')
  const ocrReceipt = readSourceReceipt(coreRoot, 'build-source-ocr.js')
  const pdfModules = ['build/pdf.mjs', 'build/pdf.worker.min.mjs', 'web/pdf_viewer.mjs']
  const pdfResources = [
    'jbig2.wasm', 'jbig2_nowasm_fallback.js', 'openjpeg.wasm', 'openjpeg_nowasm_fallback.js',
    'qcms_bg.wasm', 'quickjs-eval.wasm', 'quickjs-eval.js',
    'LICENSE_PDFJS_QCMS_SOURCE', 'LICENSE_QUICKJS',
  ]
  verifySourceFiles(pdfBuild, pdfReceipt, [
    ...pdfModules,
    ...new Set([...pdfResources, ...readdirSync(wasmRoot)].map(file => 'web/wasm/' + file)),
  ])
  verifySourceFiles(coreRoot, ocrReceipt, coreFiles)
  console.log('[pdf/ocr] verified source-built PDF.js and LSTM engine outputs')
}
preparePdfJsAssets()
prepareTesseractAssets()

function preparePdfJsAssets() {
  const outputRoot = join(publicRoot, 'pdfjs')
  const fonts = join(npmPdfRoot, 'standard_fonts')

  requirePaths([fonts, wasmRoot], 'PDF.js')
  rmSync(outputRoot, {
    recursive: true,
    force: true,
  })
  cpSync(fonts, join(outputRoot, 'standard_fonts'), { recursive: true })
  cpSync(wasmRoot, join(outputRoot, 'wasm'), { recursive: true })
  console.log('[pdfjs] prepared standard_fonts and wasm runtime assets')
}

/** Copy only the browser worker, LSTM cores, and supported offline languages. */
function prepareTesseractAssets() {
  const workerSource = join(repoRoot, 'node_modules', 'tesseract.js', 'dist', 'worker.min.js')
  const languages = ['eng', 'ara']
  const languageSources = languages.map((language) => join(
    repoRoot, 'node_modules', '@tesseract.js-data', language,
    '4.0.0_best_int', `${language}.traineddata.gz`,
  ))
  const outputRoot = join(publicRoot, 'tesseract')
  requirePaths([
    workerSource,
    ...languageSources,
    ...coreFiles.map((file) => join(coreRoot, file)),
  ], 'Tesseract')
  rmSync(outputRoot, {
    recursive: true,
    force: true,
  })
  mkdirSync(join(outputRoot, 'core'), { recursive: true })
  mkdirSync(join(outputRoot, 'lang'), { recursive: true })
  cpSync(workerSource, join(outputRoot, 'worker.min.js'))
  languages.forEach((language, index) => {
    cpSync(languageSources[index], join(outputRoot, 'lang', `${language}.traineddata.gz`))
  })
  for (const file of coreFiles) {
    cpSync(join(coreRoot, file), join(outputRoot, 'core', file))
  }
  console.log('[tesseract] prepared worker, LSTM cores, and English/Arabic language data')
}

function readSourceReceipt(directory, recipe) {
  const path = join(directory, 'source-receipt.json')
  assert(existsSync(path), `Missing source-build receipt: ${path}. Run node scripts/fdroid/${recipe} first.`)
  return JSON.parse(readFileSync(path, 'utf8'))
}

function verifySourceFiles(directory, receipt, files) {
  for (const file of files) {
    const path = join(directory, file)
    const bytes = readFileSync(path)
    assert.deepEqual(receipt.artifacts[file], {
      bytes: bytes.length,
      sha256: createHash('sha256').update(bytes).digest('hex'),
    }, `Source-built file does not match its receipt: ${path}. Rerun the source recipe.`)
  }
}

function requirePaths(paths, label) {
  for (const path of paths) {
    if (!existsSync(path)) {
      throw new Error(`Missing ${label} runtime asset: ${path}. Run npm install first.`)
    }
  }
}
