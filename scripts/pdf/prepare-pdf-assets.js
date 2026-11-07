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
// These files match the Apache-2.0 naptha/tessdata revision recorded in the source inventory.
const languageSources = [
  ['eng', '45b4cb346724ac1774f1c36f42f182b887bcdb28ebe63e6fff90ac41f3fcff91'],
  ['ara', 'f4746c44b02342dd5b3d4f0198000f47d7c49f1a229e63e0f436c0592dcd9639'],
].map(([language, sha256]) => ({
  language,
  sha256,
  path: join(repoRoot, 'node_modules', '@tesseract.js-data', language, '4.0.0_best_int', language + '.traineddata.gz'),
}))
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
  assert.equal(pdfReceipt.tools.filePrefix, '/build/papercut-pdf', 'Rerun build-source-pdf-codecs.js to normalize embedded source paths')
  assert.equal(ocrReceipt.tools.filePrefix, '/build/papercut-ocr', 'Rerun build-source-ocr.js to normalize embedded source paths')
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
  for (const { path, sha256 } of languageSources) {
    assert.equal(createHash('sha256').update(readFileSync(path)).digest('hex'), sha256, `OCR data differs from pinned tessdata: ${path}`)
  }
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
  const outputRoot = join(publicRoot, 'tesseract')
  requirePaths([
    workerSource,
    ...languageSources.map(({ path }) => path),
    ...coreFiles.map((file) => join(coreRoot, file)),
  ], 'Tesseract')
  rmSync(outputRoot, {
    recursive: true,
    force: true,
  })
  mkdirSync(join(outputRoot, 'core'), { recursive: true })
  mkdirSync(join(outputRoot, 'lang'), { recursive: true })
  cpSync(workerSource, join(outputRoot, 'worker.min.js'))
  for (const { language, path } of languageSources) {
    cpSync(path, join(outputRoot, 'lang', `${language}.traineddata.gz`))
  }
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
